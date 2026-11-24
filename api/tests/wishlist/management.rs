use crate::send_request_with_method::send_request_with_method;
use crate::support::{list, owner, request};
use crate::test_app_with_db::test_app_with_db;
use axum::http::Method;
use axum::http::StatusCode;
use serde_json::Value;
use serde_json::json;

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
        for method in [Method::GET, Method::PATCH, Method::DELETE] {
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

#[tokio::test]
async fn rich_items_validate_patch_clear_and_keep_addition_identity() {
    let (app, _) = test_app_with_db().await;
    let member = owner(&app, "Member A").await;
    let ideas = list(&app, &member, "Ideas").await;
    let payload = json!({"owner_id":member["id"],"list_id":ideas["id"],"request_id":"rich-1",
        "title":"A warm jacket", "url":"https://example.com/jacket", "image_url":"https://example.com/jacket.png",
        "notes":"For winter\nWaterproof", "price":"DKK 199.95", "quantity":2,"priority":"high"});
    let (status, added) = request(&app, Method::POST, "/api/wishlist/items", payload.clone()).await;
    assert_eq!(status, StatusCode::CREATED, "{added}");
    for field in ["url", "image_url", "notes", "price", "quantity", "priority"] {
        assert_eq!(added["item"][field], payload[field]);
    }
    let path = format!(
        "/api/household-users/{}/wishlists/{}/items/{}",
        member["id"].as_str().unwrap(),
        ideas["id"].as_str().unwrap(),
        added["item"]["id"].as_str().unwrap()
    );
    for invalid in [
        json!({"quantity":0}),
        json!({"quantity":1000}),
        json!({"quantity":1.5}),
        json!({"priority":"urgent"}),
        json!({"url":"javascript:alert(1)"}),
        json!({"image_url":"https://user:pass@example.com/a"}),
        json!({"notes":"bad\u{0000}text"}),
        json!({"price":""}),
        json!({"title":null}),
    ] {
        let (status, error) = request(&app, Method::PATCH, &path, invalid).await;
        assert!(status.is_client_error(), "{error}");
    }
    let (_, unchanged) = request(&app, Method::GET, &path, Value::Null).await;
    assert_eq!(unchanged, added["item"]);
    let (status, edited) = request(
        &app,
        Method::PATCH,
        &path,
        json!({"title":"A rain jacket","url":null,"notes":null,"quantity":3}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{edited}");
    assert_eq!(edited["url"], Value::Null);
    assert_eq!(edited["notes"], Value::Null);
    assert_eq!(edited["image_url"], payload["image_url"]);
    assert_eq!(edited["price"], payload["price"]);
    assert_eq!(edited["priority"], "high");
    let (_, retried) = request(&app, Method::POST, "/api/wishlist/items", payload.clone()).await;
    assert_eq!(retried["item"], edited);
    assert_eq!(retried["replayed"], true);
    let mut changed = payload.clone();
    changed["quantity"] = json!(3);
    assert_eq!(
        request(&app, Method::POST, "/api/wishlist/items", changed)
            .await
            .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        request(&app, Method::DELETE, &path, Value::Null).await.0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        request(&app, Method::POST, "/api/wishlist/items", payload)
            .await
            .0,
        StatusCode::CONFLICT
    );
}
