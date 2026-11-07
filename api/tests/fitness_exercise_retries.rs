//! Duplicate-safe Exercise creation (`request_id`): the contract the Android outbox retries against.

mod common;

use axum::Router;
use common::send_request_with_method;
use http::{Method, StatusCode};
use serde_json::{Value, json};
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

const KEY: &str = "phone-exercise-1";
const NAME: &str = "Offline Ring Row";

fn exercise(request_id: &str) -> Value {
    json!({
        "request_id": request_id,
        "name": NAME,
        "category": "calisthenics",
        "muscle_group": "back",
        "equipment": "rings"
    })
}

async fn post(app: &Router, body: Value) -> (StatusCode, Value) {
    send_request_with_method(
        app.clone(),
        "/api/exercises",
        Method::POST,
        Some(body),
        Some("test-api-key"),
    )
    .await
}

/// How many Exercises hold `name`, read back through the public list.
async fn named(app: &Router, name: &str) -> usize {
    let (status, body) = send_request_with_method(
        app.clone(),
        "/api/exercises",
        Method::GET,
        None,
        Some("test-api-key"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    body.as_array()
        .unwrap()
        .iter()
        .filter(|exercise| exercise["name"] == name)
        .count()
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
async fn retry_after_a_lost_response_survives_an_api_restart_as_the_same_exercise() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("app.db");

    let db = open_file_database(&path).await;
    let (app, _) = common::test_app_with_pool(db.clone(), None);
    let (status, first) = post(&app, exercise(KEY)).await;
    assert_eq!(status, StatusCode::CREATED, "{first}");
    assert_eq!(first["name"], NAME);
    assert_eq!(first["muscle_group"], "back");
    assert_eq!(first["equipment"], "rings");
    // The client never saw that answer, and the API goes away with its in-memory state.
    db.close().await;

    let db = open_file_database(&path).await;
    let (app, _) = common::test_app_with_pool(db, None);
    let (status, retried) = post(&app, exercise(KEY)).await;

    assert_eq!(status, StatusCode::OK, "{retried}");
    assert_eq!(retried, first);
    assert_eq!(named(&app, NAME).await, 1);
}

#[tokio::test]
async fn simultaneous_deliveries_create_one_exercise() {
    let dir = tempfile::tempdir().unwrap();
    let db = open_file_database(&dir.path().join("app.db")).await;
    let (app, _) = common::test_app_with_pool(db, None);

    let (one, two, three) = tokio::join!(
        post(&app, exercise(KEY)),
        post(&app, exercise(KEY)),
        post(&app, exercise(KEY)),
    );

    let mut statuses = [one.0, two.0, three.0];
    statuses.sort();
    assert_eq!(
        statuses,
        [StatusCode::OK, StatusCode::OK, StatusCode::CREATED]
    );
    assert_eq!(one.1["id"], two.1["id"]);
    assert_eq!(two.1["id"], three.1["id"]);
    assert_eq!(named(&app, NAME).await, 1);
}

#[tokio::test]
async fn request_id_with_changed_arguments_conflicts_and_changes_nothing() {
    let app = common::test_app().await;
    let (_, first) = post(&app, exercise(KEY)).await;

    let mut changed = exercise(KEY);
    changed["name"] = json!("Another Name");
    let (status, body) = post(&app, changed).await;

    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(named(&app, NAME).await, 1);
    assert_eq!(named(&app, "Another Name").await, 0);
    let (status, retried) = post(&app, exercise(KEY)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(retried["id"], first["id"]);
}

#[tokio::test]
async fn a_name_another_exercise_holds_conflicts_and_consumes_no_request_id() {
    let app = common::test_app().await;
    let (_, _) = post(&app, exercise("phone-exercise-other")).await;

    let (status, body) = post(&app, exercise(KEY)).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(named(&app, NAME).await, 1);

    // Once the name is corrected, the id that was refused can still create it.
    let mut renamed = exercise(KEY);
    renamed["name"] = json!("Offline Ring Row 2");
    let (status, body) = post(&app, renamed).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
}

#[tokio::test]
async fn an_invalid_category_consumes_no_request_id_and_writes_nothing() {
    let app = common::test_app().await;

    let mut invalid = exercise(KEY);
    invalid["category"] = json!("cardio");
    let (status, _) = post(&app, invalid).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(named(&app, NAME).await, 0);

    let (status, _) = post(&app, exercise(KEY)).await;
    assert_eq!(status, StatusCode::CREATED);
}

#[tokio::test]
async fn an_old_retry_cannot_recreate_a_deleted_exercise_or_claim_its_reused_id() {
    let app = common::test_app().await;
    let (_, first) = post(&app, exercise(KEY)).await;
    let (status, _) = send_request_with_method(
        app.clone(),
        &format!("/api/exercises/{}", first["id"]),
        Method::DELETE,
        None,
        Some("test-api-key"),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    // SQLite hands the highest id out again, so this Exercise takes the deleted one's id.
    let mut other = exercise("phone-exercise-2");
    other["name"] = json!("Took Its Id");
    let (status, other) = post(&app, other).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(other["id"], first["id"]);

    let (status, body) = post(&app, exercise(KEY)).await;

    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(named(&app, NAME).await, 0);
}

#[tokio::test]
async fn request_id_is_validated_and_requires_authentication() {
    let app = common::test_app().await;

    let (status, _) = post(&app, exercise("   ")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, _) = send_request_with_method(
        app.clone(),
        "/api/exercises",
        Method::POST,
        Some(exercise(KEY)),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(named(&app, NAME).await, 0);
}

#[tokio::test]
async fn exercises_without_a_request_id_behave_as_before() {
    let app = common::test_app().await;
    let mut body = exercise(KEY);
    body.as_object_mut().unwrap().remove("request_id");

    let (one, _) = post(&app, body.clone()).await;
    let (two, _) = post(&app, body).await;

    assert_eq!((one, two), (StatusCode::CREATED, StatusCode::CONFLICT));
    assert_eq!(named(&app, NAME).await, 1);
}
