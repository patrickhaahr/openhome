//! Duplicate-safe Workout creation (`request_id`): the contract the Android outbox retries against.

mod common;

use axum::Router;
use common::send_request_with_method;
use http::{Method, StatusCode};
use serde_json::{Value, json};
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

const KEY: &str = "phone-workout-1";

fn workout(request_id: &str) -> Value {
    json!({
        "request_id": request_id,
        "date": "2026-10-30",
        "name": "Offline session",
        "exercises": [
            {
                "exercise_id": 11,
                "order_index": 0,
                "sets": [
                    { "set_number": 1, "reps": 8, "weight_kg": 12.5, "rpe": 8 },
                    { "set_number": 2, "reps": 6 }
                ]
            },
            {
                "exercise_id": 1,
                "order_index": 1,
                "sets": [{ "set_number": 1, "duration_seconds": 20 }]
            }
        ]
    })
}

async fn post(app: &Router, body: Value) -> (StatusCode, Value) {
    send_request_with_method(
        app.clone(),
        "/api/workouts",
        Method::POST,
        Some(body),
        Some("test-api-key"),
    )
    .await
}

async fn list(app: &Router) -> Vec<Value> {
    let (status, body) = send_request_with_method(
        app.clone(),
        "/api/workouts",
        Method::GET,
        None,
        Some("test-api-key"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    body.as_array().unwrap().clone()
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
async fn retry_after_a_lost_response_survives_an_api_restart_as_the_same_workout() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("app.db");

    let db = open_file_database(&path).await;
    let (app, _) = common::test_app_with_pool(db.clone(), None);
    let (status, first) = post(&app, workout(KEY)).await;
    assert_eq!(status, StatusCode::CREATED, "{first}");
    // The client never saw that answer, and the API goes away with its in-memory state.
    db.close().await;

    let db = open_file_database(&path).await;
    let (app, _) = common::test_app_with_pool(db, None);
    let (status, retried) = post(&app, workout(KEY)).await;
    assert_eq!(status, StatusCode::OK, "{retried}");
    assert_eq!(retried, first);

    let listed = list(&app).await;
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0]["id"], first["id"]);
    let (_, read) = send_request_with_method(
        app,
        &format!("/api/workouts/{}", first["id"]),
        Method::GET,
        None,
        Some("test-api-key"),
    )
    .await;
    assert_eq!(read["exercises"].as_array().unwrap().len(), 2);
    assert_eq!(read["exercises"][0]["sets"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn simultaneous_deliveries_create_one_workout() {
    let dir = tempfile::tempdir().unwrap();
    let db = open_file_database(&dir.path().join("app.db")).await;
    let (app, _) = common::test_app_with_pool(db, None);

    let (one, two, three) = tokio::join!(
        post(&app, workout(KEY)),
        post(&app, workout(KEY)),
        post(&app, workout(KEY)),
    );

    let mut statuses = [one.0, two.0, three.0];
    statuses.sort();
    assert_eq!(
        statuses,
        [StatusCode::OK, StatusCode::OK, StatusCode::CREATED]
    );
    assert_eq!(one.1["id"], two.1["id"]);
    assert_eq!(two.1["id"], three.1["id"]);
    assert_eq!(list(&app).await.len(), 1);
}

#[tokio::test]
async fn request_id_with_changed_arguments_conflicts_and_changes_nothing() {
    let app = common::test_app().await;
    let (_, first) = post(&app, workout(KEY)).await;

    let mut changed = workout(KEY);
    changed["exercises"][0]["sets"][0]["reps"] = json!(9);
    let (status, body) = post(&app, changed).await;

    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    let (_, read) = send_request_with_method(
        app.clone(),
        &format!("/api/workouts/{}", first["id"]),
        Method::GET,
        None,
        Some("test-api-key"),
    )
    .await;
    assert_eq!(read["exercises"][0]["sets"][0]["reps"], 8);
    assert_eq!(list(&app).await.len(), 1);
}

#[tokio::test]
async fn a_rejected_request_consumes_no_request_id_and_writes_nothing() {
    let app = common::test_app().await;

    let mut unknown_exercise = workout(KEY);
    unknown_exercise["exercises"][0]["exercise_id"] = json!(424_242);
    let (status, _) = post(&app, unknown_exercise).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(list(&app).await.is_empty());

    let (status, _) = post(&app, workout(KEY)).await;
    assert_eq!(status, StatusCode::CREATED);
}

#[tokio::test]
async fn an_old_retry_cannot_recreate_a_deleted_workout_or_claim_its_reused_id() {
    let app = common::test_app().await;
    let (_, first) = post(&app, workout(KEY)).await;
    let (status, _) = send_request_with_method(
        app.clone(),
        &format!("/api/workouts/{}", first["id"]),
        Method::DELETE,
        None,
        Some("test-api-key"),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    // SQLite hands the highest id out again, so this Workout takes the deleted one's id.
    let (status, other) = post(&app, workout("phone-workout-2")).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(other["id"], first["id"]);

    let (status, body) = post(&app, workout(KEY)).await;

    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(list(&app).await.len(), 1);
}

#[tokio::test]
async fn request_id_is_validated_and_requires_authentication() {
    let app = common::test_app().await;

    let (status, _) = post(&app, workout("   ")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, _) = send_request_with_method(
        app.clone(),
        "/api/workouts",
        Method::POST,
        Some(workout(KEY)),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(list(&app).await.is_empty());
}

#[tokio::test]
async fn workouts_without_a_request_id_keep_creating_a_new_workout_each_call() {
    let app = common::test_app().await;
    let mut body = workout(KEY);
    body.as_object_mut().unwrap().remove("request_id");

    let (one, _) = post(&app, body.clone()).await;
    let (two, _) = post(&app, body).await;

    assert_eq!((one, two), (StatusCode::CREATED, StatusCode::CREATED));
    assert_eq!(list(&app).await.len(), 2);
}
