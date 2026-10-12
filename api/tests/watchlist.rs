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

fn entry_path(owner: &Value, entry: &Value) -> String {
    format!(
        "/api/household-users/{}/watchlist/entries/{}",
        owner["id"].as_str().unwrap(),
        entry["id"].as_str().unwrap()
    )
}

#[tokio::test]
async fn manual_plans_persist_and_owner_targeted_removal_cannot_change_another_list() {
    let (app, state) = test_app_with_db().await;
    let first = owner(&app, "Member A").await;
    let second = owner(&app, "Member B").await;
    let mut entries = Vec::new();
    for (member, kind, request_id) in [
        (&first, "movie", "movie-1"),
        (&second, "series", "series-1"),
    ] {
        let (status, added) = request(&app, Method::POST, "/api/watchlist/entries", json!({"owner_id":member["id"],"kind":kind,"title":" The Lighthouse Journey ","request_id":request_id})).await;
        assert_eq!(status, StatusCode::CREATED, "{added}");
        let entry = &added["entry"];
        assert_eq!(entry["owner_id"], member["id"]);
        assert_eq!(entry["kind"], kind);
        assert_eq!(entry["title"], "The Lighthouse Journey");
        assert_eq!(entry["status"], "planned");
        uuid::Uuid::parse_str(entry["id"].as_str().unwrap()).unwrap();
        let date =
            chrono::NaiveDate::parse_from_str(entry["date_added"].as_str().unwrap(), "%Y-%m-%d")
                .unwrap();
        assert_eq!(
            date,
            chrono::Utc::now()
                .with_timezone(&chrono_tz::Europe::Copenhagen)
                .date_naive()
        );
        assert_eq!(
            request(&app, Method::GET, &entry_path(member, entry), Value::Null).await,
            (StatusCode::OK, entry.clone())
        );
        entries.push(entry.clone());
    }
    assert_eq!(
        request(&app, Method::GET, "/api/watchlist/entries", Value::Null)
            .await
            .1
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM watchlist_entries")
            .fetch_one(&state.db)
            .await
            .unwrap(),
        2
    );
    for method in [Method::GET, Method::DELETE] {
        assert_eq!(
            request(&app, method, &entry_path(&second, &entries[0]), Value::Null)
                .await
                .0,
            StatusCode::NOT_FOUND
        );
    }
    assert_eq!(
        request(
            &app,
            Method::DELETE,
            &entry_path(&first, &entries[0]),
            Value::Null
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        request(&app, Method::GET, "/api/watchlist/entries", Value::Null)
            .await
            .1,
        json!([entries[1]])
    );
    assert_eq!(
        request(
            &app,
            Method::DELETE,
            &entry_path(&first, &entries[0]),
            Value::Null
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    // Existing training records retain their shared schema and endpoint.
    assert_eq!(
        request(&app, Method::GET, "/api/workouts", Value::Null)
            .await
            .0,
        StatusCode::OK
    );
    request(
        &app,
        Method::DELETE,
        &format!("/api/household-users/{}", second["id"].as_str().unwrap()),
        Value::Null,
    )
    .await;
    assert_eq!(
        request(&app, Method::GET, "/api/watchlist/entries", Value::Null)
            .await
            .1,
        json!([])
    );
}

#[tokio::test]
async fn missing_owner_requires_clarification_without_consuming_the_request_id() {
    let (app, _) = test_app_with_db().await;
    let payload = json!({"title":"A Quiet Orbit","kind":"movie","request_id":"clarify"});
    let first = owner(&app, "Member A").await;
    // Even a single household member is not inferred ownership.
    let (status, clarification) = request(
        &app,
        Method::POST,
        "/api/watchlist/entries",
        payload.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(clarification["code"], "watchlist_destination_required");
    assert!(
        clarification["question"]
            .as_str()
            .unwrap()
            .contains("owner")
    );
    assert_eq!(clarification["choices"], json!([first]));
    assert_eq!(
        request(&app, Method::GET, "/api/watchlist/entries", Value::Null)
            .await
            .1,
        json!([])
    );
    let mut selected = payload;
    selected["owner_id"] = first["id"].clone();
    assert_eq!(
        request(&app, Method::POST, "/api/watchlist/entries", selected)
            .await
            .0,
        StatusCode::CREATED
    );
}

#[tokio::test]
async fn retries_are_atomic_and_receipts_survive_entry_and_owner_deletion() {
    let (app, _) = test_app_with_db().await;
    let first = owner(&app, "Member A").await;
    let second = owner(&app, "Member B").await;
    let payload = json!({"owner_id":first["id"],"title":" A Quiet Orbit ","kind":"movie","request_id":"retry"});
    let (left, right) = tokio::join!(
        request(
            &app,
            Method::POST,
            "/api/watchlist/entries",
            payload.clone()
        ),
        request(
            &app,
            Method::POST,
            "/api/watchlist/entries",
            payload.clone()
        )
    );
    assert!([left.0, right.0].contains(&StatusCode::CREATED));
    assert!([left.0, right.0].contains(&StatusCode::OK));
    assert_eq!(left.1["entry"], right.1["entry"]);
    let mut retry = payload;
    retry["title"] = json!("A Quiet Orbit");
    assert_eq!(
        request(&app, Method::POST, "/api/watchlist/entries", retry.clone())
            .await
            .1,
        json!({"entry":left.1["entry"],"replayed":true})
    );
    for (field, value) in [
        ("title", json!("Another title")),
        ("kind", json!("series")),
        ("owner_id", second["id"].clone()),
    ] {
        let mut changed = retry.clone();
        changed[field] = value;
        assert_eq!(
            request(&app, Method::POST, "/api/watchlist/entries", changed)
                .await
                .0,
            StatusCode::CONFLICT
        );
    }
    assert_eq!(
        request(
            &app,
            Method::DELETE,
            &entry_path(&first, &left.1["entry"]),
            Value::Null
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        request(&app, Method::POST, "/api/watchlist/entries", retry.clone())
            .await
            .0,
        StatusCode::CONFLICT
    );
    retry["request_id"] = json!("new-intention");
    assert_eq!(
        request(&app, Method::POST, "/api/watchlist/entries", retry.clone())
            .await
            .0,
        StatusCode::CREATED
    );
    request(
        &app,
        Method::DELETE,
        &format!("/api/household-users/{}", first["id"].as_str().unwrap()),
        Value::Null,
    )
    .await;
    assert_eq!(
        request(&app, Method::POST, "/api/watchlist/entries", retry)
            .await
            .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        request(&app, Method::GET, "/api/watchlist/entries", Value::Null)
            .await
            .1,
        json!([])
    );
}

#[tokio::test]
async fn authenticated_routes_reject_invalid_manual_plans_without_writing() {
    let (app, _) = test_app_with_db().await;
    let member = owner(&app, "Member A").await;
    let payload = json!({"owner_id":member["id"],"title":"A Quiet Orbit","kind":"movie","request_id":"validation"});
    for (method, path) in [
        (Method::GET, "/api/watchlist/entries"),
        (Method::POST, "/api/watchlist/entries"),
        (
            Method::GET,
            "/api/household-users/missing/watchlist/entries/missing",
        ),
        (
            Method::DELETE,
            "/api/household-users/missing/watchlist/entries/missing",
        ),
    ] {
        for authorization in [None, Some("wrong-key")] {
            assert_eq!(
                send_request_with_method(
                    app.clone(),
                    path,
                    method.clone(),
                    Some(payload.clone()),
                    authorization
                )
                .await
                .0,
                StatusCode::UNAUTHORIZED
            );
        }
    }
    for (field, value) in [
        ("title", json!(" ")),
        ("title", json!("x".repeat(501))),
        ("title", json!("bad\nname")),
        ("request_id", json!("")),
        ("request_id", json!("x".repeat(129))),
        ("kind", json!("episode")),
        ("status", json!("completed")),
        ("date_added", json!("2020-01-01")),
    ] {
        let mut invalid = payload.clone();
        invalid[field] = value;
        assert!(
            request(&app, Method::POST, "/api/watchlist/entries", invalid)
                .await
                .0
                .is_client_error()
        );
    }
    let mut missing = payload.clone();
    missing["owner_id"] = json!("missing");
    assert_eq!(
        request(&app, Method::POST, "/api/watchlist/entries", missing)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        request(&app, Method::GET, "/api/watchlist/entries", Value::Null)
            .await
            .1,
        json!([])
    );
    assert_eq!(
        request(&app, Method::POST, "/api/watchlist/entries", payload)
            .await
            .0,
        StatusCode::CREATED
    );
}
