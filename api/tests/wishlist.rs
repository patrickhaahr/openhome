mod common;

use axum::{
    Router,
    http::{Method, StatusCode},
};
use common::{send_request_with_method, test_app_with_db};
use serde_json::{Value, json};

async fn request(app: &Router, method: Method, path: &str, body: Value) -> (StatusCode, Value) {
    send_request_with_method(app.clone(), path, method, Some(body), Some("test-api-key")).await
}

async fn owner(app: &Router, name: &str) -> Value {
    let (status, body) = request(
        app,
        Method::POST,
        "/api/household-users",
        json!({"name":name}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body
}

async fn list(app: &Router, owner: &Value, name: &str) -> Value {
    let (status, body) = request(
        app,
        Method::POST,
        "/api/wishlists",
        json!({"owner_id":owner["id"],"name":name}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body
}

#[tokio::test]
async fn generic_wishes_are_shared_persisted_and_retries_do_not_duplicate() {
    let (app, _) = test_app_with_db().await;
    let first = owner(&app, "Member A").await;
    let second = owner(&app, "Member B").await;
    let first_list = list(&app, &first, "Ideas").await;
    let second_list = list(&app, &second, "Ideas").await;
    let payload =
        json!({"list_id":first_list["id"],"title":" A warm jacket ","request_id":"wish-1"});
    let (status, added) = request(&app, Method::POST, "/api/wishlist/items", payload.clone()).await;
    assert_eq!(status, StatusCode::CREATED, "{added}");
    assert_eq!(added["item"]["title"], "A warm jacket");
    assert_eq!(added["item"]["list_id"], first_list["id"]);
    let (status, retried) =
        request(&app, Method::POST, "/api/wishlist/items", payload.clone()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(retried["item"], added["item"]);
    assert_eq!(retried["replayed"], true);
    let mut conflict = payload;
    conflict["list_id"] = second_list["id"].clone();
    assert_eq!(
        request(&app, Method::POST, "/api/wishlist/items", conflict)
            .await
            .0,
        StatusCode::CONFLICT
    );
    let (_, owners) = request(&app, Method::GET, "/api/household-users", Value::Null).await;
    assert_eq!(owners.as_array().unwrap().len(), 2);
    let (_, lists) = request(&app, Method::GET, "/api/wishlists", Value::Null).await;
    assert_eq!(lists.as_array().unwrap().len(), 2);
    for (list, expected) in [
        (&first_list, json!([added["item"]])),
        (&second_list, json!([])),
    ] {
        let (status, items) = request(
            &app,
            Method::GET,
            &format!("/api/wishlists/{}/items", list["id"].as_str().unwrap()),
            Value::Null,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(items, expected);
    }
}

#[tokio::test]
async fn ambiguous_destinations_clarify_without_writing_and_explicit_owner_is_checked() {
    let (app, _) = test_app_with_db().await;
    let first = owner(&app, "Member A").await;
    let second = owner(&app, "Member B").await;
    let target = list(&app, &first, "Ideas").await;
    list(&app, &first, "Outdoors").await;
    let payload = json!({"title":"A warm jacket","request_id":"wish-2","owner_id":first["id"]});
    let (status, error) = request(&app, Method::POST, "/api/wishlist/items", payload.clone()).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(error["code"], "wishlist_destination_required");
    assert_eq!(error["choices"].as_array().unwrap().len(), 2);
    let mut selected = payload;
    selected["list_id"] = target["id"].clone();
    selected["owner_id"] = second["id"].clone();
    assert_eq!(
        request(&app, Method::POST, "/api/wishlist/items", selected.clone())
            .await
            .0,
        StatusCode::CONFLICT
    );
    selected["owner_id"] = first["id"].clone();
    assert_eq!(
        request(&app, Method::POST, "/api/wishlist/items", selected)
            .await
            .0,
        StatusCode::CREATED
    );
}

#[tokio::test]
async fn validates_inputs_and_authentication_before_writes() {
    let (app, _) = test_app_with_db().await;
    for authorization in [None, Some("wrong-key")] {
        assert_eq!(
            send_request_with_method(
                app.clone(),
                "/api/household-users",
                Method::POST,
                Some(json!({"name":"Member A"})),
                authorization
            )
            .await
            .0,
            StatusCode::UNAUTHORIZED
        );
    }
    for name in ["", "   ", &"x".repeat(201)] {
        assert_eq!(
            request(
                &app,
                Method::POST,
                "/api/household-users",
                json!({"name":name})
            )
            .await
            .0,
            StatusCode::BAD_REQUEST
        );
    }
    let member = owner(&app, "Member A").await;
    list(&app, &member, "Ideas").await;
    assert_eq!(
        request(
            &app,
            Method::POST,
            "/api/wishlists",
            json!({"owner_id":member["id"],"name":"ideas"})
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    for payload in [
        json!({"title":" ","request_id":"a"}),
        json!({"title":"Jacket","request_id":" "}),
        json!({"title":"Jacket","request_id":"a","url":"https://example.com"}),
    ] {
        assert!(
            request(&app, Method::POST, "/api/wishlist/items", payload)
                .await
                .0
                .is_client_error()
        );
    }
    let payload = json!({"owner_id":member["id"],"title":"Jacket","request_id":"a"});
    assert_eq!(
        request(&app, Method::POST, "/api/wishlist/items", payload)
            .await
            .0,
        StatusCode::CREATED
    );
}

fn configured_app(state: openhome_api::AppState, default: Option<String>) -> Router {
    let key = openhome_api::auth::ApiKey::new("test-api-key".into());
    openhome_api::routes::wishlist::router(default)
        .with_state(state)
        .layer(axum::middleware::from_fn(move |request, next| {
            openhome_api::auth::auth_middleware(request, next, key.clone())
        }))
}

#[tokio::test]
async fn explicit_destinations_override_defaults_and_accepted_retries_survive_default_changes() {
    let (app, state) = test_app_with_db().await;
    let first = owner(&app, "Member A").await;
    let second = owner(&app, "Member B").await;
    let first_list = list(&app, &first, "Ideas").await;
    let second_list = list(&app, &second, "Ideas").await;
    let default_app = configured_app(
        state.clone(),
        Some(first_list["id"].as_str().unwrap().into()),
    );
    let payload = json!({"title":"A warm jacket","request_id":"default-1"});
    let (status, added) = request(
        &default_app,
        Method::POST,
        "/api/wishlist/items",
        payload.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(added["item"]["list_id"], first_list["id"]);
    let changed_default = configured_app(
        state.clone(),
        Some(second_list["id"].as_str().unwrap().into()),
    );
    let (_, replayed) = request(
        &changed_default,
        Method::POST,
        "/api/wishlist/items",
        payload.clone(),
    )
    .await;
    assert_eq!(replayed["item"], added["item"]);
    let (_, replayed) = request(&app, Method::POST, "/api/wishlist/items", payload).await;
    assert_eq!(replayed["item"], added["item"]);
    let (_, explicit) = request(
        &default_app,
        Method::POST,
        "/api/wishlist/items",
        json!({"title":"A book","request_id":"explicit-1","owner_id":second["id"]}),
    )
    .await;
    assert_eq!(explicit["item"]["list_id"], second_list["id"]);
    let invalid_default = configured_app(state, Some("missing-list".into()));
    assert_eq!(
        request(
            &invalid_default,
            Method::POST,
            "/api/wishlist/items",
            json!({"title":"A book","request_id":"invalid-1"})
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn concurrent_additions_claim_one_request_id_and_list_deletion_leaves_no_orphans() {
    let (app, state) = test_app_with_db().await;
    let member = owner(&app, "Member A").await;
    let target = list(&app, &member, "Ideas").await;
    let payload =
        json!({"title":"A warm jacket","request_id":"concurrent-1","list_id":target["id"]});
    let (first, second) = tokio::join!(
        request(&app, Method::POST, "/api/wishlist/items", payload.clone()),
        request(&app, Method::POST, "/api/wishlist/items", payload)
    );
    let mut statuses = [first.0.as_u16(), second.0.as_u16()];
    statuses.sort_unstable();
    assert_eq!(statuses, [200, 201]);
    assert_eq!(first.1["item"], second.1["item"]);
    sqlx::query("DELETE FROM wishlists WHERE id = ?")
        .bind(target["id"].as_str().unwrap())
        .execute(&state.db)
        .await
        .unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM wishlist_items")
        .fetch_one(&state.db)
        .await
        .unwrap();
    assert_eq!(count, 0, "migration must cascade item deletion");
}

#[tokio::test]
async fn database_failures_are_never_reported_as_successful_writes() {
    let (app, state) = test_app_with_db().await;
    state.db.close().await;
    let (status, error) = request(
        &app,
        Method::POST,
        "/api/household-users",
        json!({"name":"Member A"}),
    )
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(
        error,
        json!({"error":"An internal error occurred","status":500})
    );
}

#[tokio::test]
async fn household_management_targets_owners_lists_and_items_and_preserves_addition_identity() {
    let (app, state) = test_app_with_db().await;
    let first = owner(&app, "Member A").await;
    let second = owner(&app, "Member B").await;
    let target = list(&app, &first, "Ideas").await;
    let other = list(&app, &second, "Ideas").await;
    let addition = json!({"owner_id":first["id"],"list_id":target["id"],"title":"A jacket","request_id":"edit-retry"});
    let (_, added) = request(&app, Method::POST, "/api/wishlist/items", addition.clone()).await;
    let owner_path = format!("/api/household-users/{}", first["id"].as_str().unwrap());
    let path = format!("{owner_path}/wishlists/{}", target["id"].as_str().unwrap());
    let item_path = format!("{path}/items/{}", added["item"]["id"].as_str().unwrap());
    let wrong_owner = path.replace(
        first["id"].as_str().unwrap(),
        second["id"].as_str().unwrap(),
    );
    let wrong_item = item_path.replace(
        target["id"].as_str().unwrap(),
        other["id"].as_str().unwrap(),
    );
    for (method, path, payload, expected) in [
        (
            Method::PATCH,
            owner_path.as_str(),
            json!({"name":" Member C "}),
            StatusCode::OK,
        ),
        (
            Method::PATCH,
            path.as_str(),
            json!({"name":" Outdoors "}),
            StatusCode::OK,
        ),
        (
            Method::PATCH,
            item_path.as_str(),
            json!({"title":" A waterproof jacket "}),
            StatusCode::OK,
        ),
        (
            Method::PATCH,
            wrong_owner.as_str(),
            json!({"name":"Wrong"}),
            StatusCode::NOT_FOUND,
        ),
        (
            Method::DELETE,
            wrong_owner.as_str(),
            Value::Null,
            StatusCode::NOT_FOUND,
        ),
        (
            Method::PATCH,
            wrong_item.as_str(),
            json!({"title":"Wrong"}),
            StatusCode::NOT_FOUND,
        ),
        (
            Method::DELETE,
            wrong_item.as_str(),
            Value::Null,
            StatusCode::NOT_FOUND,
        ),
        (
            Method::PATCH,
            item_path.as_str(),
            json!({"title":" "}),
            StatusCode::BAD_REQUEST,
        ),
        (
            Method::PATCH,
            owner_path.as_str(),
            json!({"name":"member b"}),
            StatusCode::CONFLICT,
        ),
    ] {
        let (status, body) = request(&app, method, path, payload).await;
        assert_eq!(status, expected, "{path}: {body}");
    }
    let (status, replayed) =
        request(&app, Method::POST, "/api/wishlist/items", addition.clone()).await;
    assert_eq!(status, StatusCode::OK, "{replayed}");
    assert_eq!(replayed["item"]["id"], added["item"]["id"]);
    assert_eq!(replayed["item"]["title"], "A waterproof jacket");
    let (_, owners) = request(&app, Method::GET, "/api/household-users", Value::Null).await;
    assert_eq!(owners[1]["name"], "Member C");
    let (_, lists) = request(&app, Method::GET, "/api/wishlists", Value::Null).await;
    assert!(
        lists
            .as_array()
            .unwrap()
            .iter()
            .any(|l| l["id"] == target["id"] && l["name"] == "Outdoors")
    );
    for path in [&item_path, &path, &owner_path] {
        assert_eq!(
            request(&app, Method::DELETE, path, Value::Null).await.0,
            StatusCode::NO_CONTENT
        );
    }
    let (status, _) = request(&app, Method::POST, "/api/wishlist/items", addition).await;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "deleted additions must not be recreated by retries"
    );
    let (_, remaining) = request(&app, Method::GET, "/api/wishlists", Value::Null).await;
    assert_eq!(remaining, json!([other]));
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM wishlist_items")
        .fetch_one(&state.db)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn cascading_removal_retires_additions_and_all_management_routes_require_authentication() {
    let (app, state) = test_app_with_db().await;
    let member = owner(&app, "Member A").await;
    let target = list(&app, &member, "Ideas").await;
    let path = format!(
        "/api/household-users/{}/wishlists/{}",
        member["id"].as_str().unwrap(),
        target["id"].as_str().unwrap()
    );
    let addition = json!({"owner_id":member["id"],"list_id":target["id"],"title":"A book","request_id":"cascade-retry"});
    let (_, item) = request(&app, Method::POST, "/api/wishlist/items", addition.clone()).await;
    let item_path = format!("{path}/items/{}", item["item"]["id"].as_str().unwrap());
    for route in [
        &path,
        &item_path,
        &format!("/api/household-users/{}", member["id"].as_str().unwrap()),
    ] {
        for method in [Method::PATCH, Method::DELETE] {
            assert_eq!(
                send_request_with_method(
                    app.clone(),
                    route,
                    method,
                    Some(json!({"name":"Changed","title":"Changed"})),
                    None
                )
                .await
                .0,
                StatusCode::UNAUTHORIZED
            );
        }
    }
    assert_eq!(
        request(&app, Method::DELETE, &path, Value::Null).await.0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        request(&app, Method::POST, "/api/wishlist/items", addition)
            .await
            .0,
        StatusCode::CONFLICT
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM wishlist_items")
        .fetch_one(&state.db)
        .await
        .unwrap();
    assert_eq!(count, 0);
    let nested = list(&app, &member, "Outdoors").await;
    request(
        &app,
        Method::POST,
        "/api/wishlist/items",
        json!({"list_id":nested["id"],"title":"Boots","request_id":"owner-cascade"}),
    )
    .await;
    request(
        &app,
        Method::DELETE,
        &format!("/api/household-users/{}", member["id"].as_str().unwrap()),
        Value::Null,
    )
    .await;
    assert_eq!(
        request(&app, Method::GET, "/api/wishlists", Value::Null)
            .await
            .1,
        json!([])
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM wishlist_items")
        .fetch_one(&state.db)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn upgrading_existing_wishlists_preserves_accepted_addition_receipts() {
    let (app, state) = test_app_with_db().await;
    sqlx::migrate!("./migrations")
        .undo(&state.db, 6)
        .await
        .unwrap();
    let member = owner(&app, "Member A").await;
    let target = list(&app, &member, "Ideas").await;
    // A pre-0007 accepted addition, as stored by the original #97 API.
    sqlx::query("INSERT INTO wishlist_items (id, list_id, title, request_id, requested_list_id) VALUES ('legacy-item', ?, 'A book', 'legacy-request', ?)")
        .bind(target["id"].as_str().unwrap())
        .bind(target["id"].as_str().unwrap())
        .execute(&state.db).await.unwrap();
    sqlx::migrate!("./migrations").run(&state.db).await.unwrap();
    let (status, replayed) = request(
        &app,
        Method::POST,
        "/api/wishlist/items",
        json!({"list_id":target["id"],"title":"A book","request_id":"legacy-request"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{replayed}");
    assert_eq!(replayed["item"]["id"], "legacy-item");
    assert_eq!(replayed["replayed"], true);
}
