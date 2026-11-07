//! Duplicate-safe Body Weight recording (`request_id`): the contract the Android outbox retries against.

mod common;

use axum::Router;
use common::send_request_with_method;
use http::{Method, StatusCode};
use serde_json::{Value, json};
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

const DATE: &str = "2026-09-05";

fn record(request_id: &str, date: &str, weight_kg: f64) -> Value {
    json!({ "request_id": request_id, "date": date, "weight_kg": weight_kg })
}

async fn post(app: &Router, body: Value) -> (StatusCode, Value) {
    send_request_with_method(
        app.clone(),
        "/api/body_weight",
        Method::POST,
        Some(body),
        Some("test-api-key"),
    )
    .await
}

/// The stored records, read back through the public list.
async fn stored(app: &Router) -> Vec<Value> {
    let (status, body) = send_request_with_method(
        app.clone(),
        "/api/body_weight",
        Method::GET,
        None,
        Some("test-api-key"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    body.as_array().unwrap().clone()
}

#[tokio::test]
async fn retrying_a_committed_record_answers_with_the_same_one_and_stores_one() {
    let app = common::test_app().await;

    let (status, first) = post(&app, record("phone-weight-1", DATE, 75.5)).await;
    assert_eq!(status, StatusCode::CREATED, "{first}");

    for _ in 0..2 {
        let (status, retry) = post(&app, record("phone-weight-1", DATE, 75.5)).await;
        assert_eq!(status, StatusCode::OK, "{retry}");
        assert_eq!(retry, first);
    }
    assert_eq!(stored(&app).await, vec![first]);
}

#[tokio::test]
async fn a_retry_with_the_date_written_unpadded_is_the_same_operation() {
    let app = common::test_app().await;

    let (status, first) = post(&app, record("phone-weight-1", "2026-09-05", 75.5)).await;
    assert_eq!(status, StatusCode::CREATED, "{first}");

    let (status, retry) = post(&app, record("phone-weight-1", "2026-9-5", 75.5)).await;
    assert_eq!(status, StatusCode::OK, "{retry}");
    assert_eq!(retry, first);
}

#[tokio::test]
async fn one_request_id_with_changed_arguments_is_a_conflict_and_changes_nothing() {
    let app = common::test_app().await;
    let (_, first) = post(&app, record("phone-weight-1", DATE, 75.5)).await;

    let (status, body) = post(&app, record("phone-weight-1", DATE, 74.0)).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(body["error"].as_str().unwrap().contains("request_id"));

    let (status, _) = post(&app, record("phone-weight-1", "2026-09-06", 75.5)).await;
    assert_eq!(status, StatusCode::CONFLICT);

    assert_eq!(stored(&app).await, vec![first]);
}

/// A genuine second entry for a date, whatever its value, keeps the existing rule.
#[tokio::test]
async fn a_new_request_id_for_a_recorded_date_is_still_a_duplicate_date() {
    let app = common::test_app().await;
    let (_, first) = post(&app, record("phone-weight-1", DATE, 75.5)).await;

    for weight in [74.0, 75.5] {
        let (status, body) = post(&app, record("phone-weight-2", DATE, weight)).await;
        assert_eq!(status, StatusCode::CONFLICT, "{body}");
        assert!(body["error"].as_str().unwrap().contains(DATE));
    }
    assert_eq!(stored(&app).await, vec![first]);
}

#[tokio::test]
async fn a_record_from_a_client_without_request_ids_still_conflicts_both_ways() {
    let app = common::test_app().await;

    let (status, first) = post(&app, json!({ "date": DATE, "weight_kg": 75.5 })).await;
    assert_eq!(status, StatusCode::CREATED);

    let (status, _) = post(&app, record("phone-weight-1", DATE, 75.5)).await;
    assert_eq!(status, StatusCode::CONFLICT);

    let (status, _) = post(&app, json!({ "date": DATE, "weight_kg": 75.5 })).await;
    assert_eq!(status, StatusCode::CONFLICT);

    assert_eq!(stored(&app).await, vec![first]);
}

#[tokio::test]
async fn a_refused_record_uses_up_no_request_id() {
    let app = common::test_app().await;

    // Refused twice over: an invalid weight, then a date another record holds.
    let (status, _) = post(&app, record("phone-weight-1", DATE, 0.0)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    post(&app, record("phone-weight-other", "2026-09-06", 75.5)).await;
    let (status, _) = post(&app, record("phone-weight-1", "2026-09-06", 75.5)).await;
    assert_eq!(status, StatusCode::CONFLICT);

    let (status, body) = post(&app, record("phone-weight-1", DATE, 75.5)).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(stored(&app).await.len(), 2);
}

#[tokio::test]
async fn an_unusable_request_id_is_refused_without_recording() {
    let app = common::test_app().await;

    let (status, body) = post(&app, record("  ", DATE, 75.5)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["error"].as_str().unwrap().contains("request_id"));
    assert!(stored(&app).await.is_empty());
}

/// Changed arguments are judged before the cleared target, so a retry whose arguments no longer
/// match reports that, not the missing record.
#[tokio::test]
async fn changed_arguments_are_refused_before_a_removed_records_target() {
    let (app, state) = common::test_app_with_db().await;
    let (status, _) = post(&app, record("phone-weight-1", DATE, 75.5)).await;
    assert_eq!(status, StatusCode::CREATED);
    sqlx::query("DELETE FROM body_weight")
        .execute(&state.db)
        .await
        .unwrap();

    let (status, body) = post(&app, record("phone-weight-1", DATE, 74.0)).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert!(
        body["error"]
            .as_str()
            .unwrap()
            .contains("a different Body Weight"),
        "{body}"
    );
    assert!(
        !body["error"].as_str().unwrap().contains("removed"),
        "{body}"
    );

    let (status, body) = post(&app, record("phone-weight-1", DATE, 75.5)).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert!(
        body["error"]
            .as_str()
            .unwrap()
            .contains("The retried Body Weight was removed"),
        "{body}"
    );
    assert!(stored(&app).await.is_empty());
}

async fn open_file_database(path: &std::path::Path) -> SqlitePool {
    let db = SqlitePoolOptions::new()
        .max_connections(4)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(path)
                .create_if_missing(true),
        )
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&db).await.unwrap();
    db
}

/// The claim serializes simultaneous deliveries: one record is stored and the losers replay it.
#[tokio::test]
async fn simultaneous_deliveries_record_one_weight() {
    let dir = tempfile::tempdir().unwrap();
    let db = open_file_database(&dir.path().join("app.db")).await;
    let (app, _) = common::test_app_with_pool(db, None);

    let (one, two, three) = tokio::join!(
        post(&app, record("phone-weight-1", DATE, 75.5)),
        post(&app, record("phone-weight-1", DATE, 75.5)),
        post(&app, record("phone-weight-1", DATE, 75.5)),
    );

    let mut statuses = [one.0, two.0, three.0];
    statuses.sort();
    assert_eq!(
        statuses,
        [StatusCode::OK, StatusCode::OK, StatusCode::CREATED]
    );
    assert_eq!(one.1["id"], two.1["id"]);
    assert_eq!(two.1["id"], three.1["id"]);
    let kept = stored(&app).await;
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0]["id"], one.1["id"]);
}

#[tokio::test]
async fn a_retry_of_a_removed_record_does_not_recreate_it() {
    let (app, state) = common::test_app_with_db().await;
    post(&app, record("phone-weight-1", DATE, 75.5)).await;
    sqlx::query("DELETE FROM body_weight")
        .execute(&state.db)
        .await
        .unwrap();

    let (status, _) = post(&app, record("phone-weight-1", DATE, 75.5)).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(stored(&app).await.is_empty());

    // The date itself is free for a new request_id.
    let (status, _) = post(&app, record("phone-weight-2", DATE, 75.5)).await;
    assert_eq!(status, StatusCode::CREATED);
}
