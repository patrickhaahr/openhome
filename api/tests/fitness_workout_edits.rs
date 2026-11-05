//! Replay-safe Workout edits (`client_revision`): the contract the Android outbox delivers against.

mod common;

use axum::Router;
use common::send_request_with_method;
use http::{Method, StatusCode};
use serde_json::{Value, json};
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

const KEY: &str = "test-api-key";

async fn create(app: &Router) -> i64 {
    let (status, body) = send_request_with_method(
        app.clone(),
        "/api/workouts",
        Method::POST,
        Some(json!({
            "date": "2026-10-30",
            "name": "Logged",
            "notes": "As logged",
            "exercises": [{
                "exercise_id": 1,
                "order_index": 0,
                "sets": [{ "set_number": 1, "reps": 5 }]
            }]
        })),
        Some(KEY),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body["id"].as_i64().unwrap()
}

/// A whole correction, as the client queues one.
fn edit(revision: i64, reps: i64) -> Value {
    json!({
        "client_revision": revision,
        "date": format!("2026-10-{:02}", 10 + revision),
        "name": format!("Revision {revision}"),
        "notes": null,
        "exercises": [{
            "exercise_id": 1,
            "order_index": 0,
            "notes": "Kept",
            "sets": [{ "set_number": 1, "reps": reps, "notes": "Also kept" }]
        }]
    })
}

async fn patch(app: &Router, id: i64, body: Value) -> (StatusCode, Value) {
    send_request_with_method(
        app.clone(),
        &format!("/api/workouts/{id}"),
        Method::PATCH,
        Some(body),
        Some(KEY),
    )
    .await
}

async fn read(app: &Router, id: i64) -> Value {
    let (status, body) = send_request_with_method(
        app.clone(),
        &format!("/api/workouts/{id}"),
        Method::GET,
        None,
        Some(KEY),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body
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

#[tokio::test]
async fn a_late_older_revision_cannot_roll_the_workout_back_even_after_an_api_restart() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("app.db");
    let db = open_file_database(&path).await;
    let (app, _) = common::test_app_with_pool(db.clone(), None);
    let id = create(&app).await;

    let (status, _) = patch(&app, id, edit(1, 6)).await;
    assert_eq!(status, StatusCode::OK);
    let (status, newest) = patch(&app, id, edit(3, 8)).await;
    assert_eq!(status, StatusCode::OK);
    db.close().await;

    let db = open_file_database(&path).await;
    let (app, _) = common::test_app_with_pool(db, None);
    let (status, answered) = patch(&app, id, edit(2, 7)).await;

    assert_eq!(status, StatusCode::OK, "{answered}");
    let current = read(&app, id).await;
    assert_eq!(answered, current);
    assert_eq!(current, newest);
    assert_eq!(current["date"], "2026-10-13");
    assert_eq!(current["exercises"][0]["sets"][0]["reps"], 8);
}

#[tokio::test]
async fn retrying_an_edit_after_a_lost_response_is_acknowledged_with_nothing_rewritten() {
    let app = common::test_app().await;
    let id = create(&app).await;
    let (_, first) = patch(&app, id, edit(1, 6)).await;

    // A different body under the same revision shows the retry applies nothing, whatever ids SQLite reuses.
    let (status, retried) = patch(&app, id, edit(1, 7)).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(retried, first);
    assert_eq!(read(&app, id).await, first);
}

#[tokio::test]
async fn a_correction_replaces_everything_it_states_and_keeps_nested_notes() {
    let app = common::test_app().await;
    let id = create(&app).await;

    let (status, body) = patch(&app, id, edit(1, 6)).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["id"], id);
    assert_eq!(body["name"], "Revision 1");
    assert_eq!(body["notes"], Value::Null);
    assert_eq!(body["exercises"][0]["notes"], "Kept");
    assert_eq!(body["exercises"][0]["sets"][0]["notes"], "Also kept");
}

#[tokio::test]
async fn a_refused_edit_uses_up_no_revision() {
    let app = common::test_app().await;
    let id = create(&app).await;

    let mut unknown_exercise = edit(1, 6);
    unknown_exercise["exercises"][0]["exercise_id"] = json!(424_242);
    let (status, _) = patch(&app, id, unknown_exercise).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let untouched = read(&app, id).await;
    assert_eq!(untouched["name"], "Logged");
    assert!(untouched.get("client_revision").is_none());

    let (status, body) = patch(&app, id, edit(1, 6)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["name"], "Revision 1");
}

#[tokio::test]
async fn an_unusable_revision_is_rejected_and_an_unknown_workout_stays_not_found() {
    let app = common::test_app().await;
    let id = create(&app).await;

    let (status, _) = patch(&app, id, edit(0, 6)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(read(&app, id).await["name"], "Logged");

    let (status, _) = patch(&app, 424_242, edit(1, 6)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_new_client_continues_from_the_revision_it_downloads() {
    let app = common::test_app().await;
    let id = create(&app).await;
    patch(&app, id, edit(4, 6)).await;

    let downloaded = read(&app, id).await;
    assert_eq!(downloaded["client_revision"], 4);

    let next = downloaded["client_revision"].as_i64().unwrap() + 1;
    let (_, body) = patch(&app, id, edit(next, 9)).await;
    assert_eq!(body["client_revision"], 5);
    assert_eq!(body["exercises"][0]["sets"][0]["reps"], 9);
}

#[tokio::test]
async fn edits_without_a_revision_apply_as_before_and_keep_partial_semantics() {
    let app = common::test_app().await;
    let id = create(&app).await;

    let (status, body) = patch(&app, id, json!({ "name": "Renamed" })).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["name"], "Renamed");
    assert_eq!(body["notes"], "As logged");
    assert_eq!(body["exercises"][0]["sets"][0]["reps"], 5);
    assert!(
        body.get("client_revision").is_none(),
        "existing clients see the same response: {body}"
    );

    // They neither need a revision nor disturb the one a queued edit relies on.
    patch(&app, id, edit(2, 6)).await;
    let (_, body) = patch(&app, id, json!({ "name": "Dashboard" })).await;
    assert_eq!(body["name"], "Dashboard");
    assert_eq!(body["client_revision"], 2);
    let (_, body) = patch(&app, id, edit(1, 7)).await;
    assert_eq!(body["name"], "Dashboard");
}
