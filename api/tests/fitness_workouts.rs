//! Logged workouts with nested exercises and sets.

mod common;

use axum::Router;
use common::send_request_with_method;
use http::Method;
use http::StatusCode;
use serde_json::json;

// ---------------------------------------------------------------------------
// Workout logging (#7)
// ---------------------------------------------------------------------------

fn sample_workout_body() -> serde_json::Value {
    json!({
        "date": "2026-08-23",
        "name": "Pull day",
        "notes": "Felt strong",
        "exercises": [
            {
                "exercise_id": 11,
                "order_index": 0,
                "notes": "grip gave out",
                "sets": [
                    { "set_number": 1, "reps": 10, "weight_kg": null, "rpe": 8 },
                    { "set_number": 2, "reps": 8, "weight_kg": 5.0, "rpe": 9 }
                ]
            },
            {
                "exercise_id": 1,
                "order_index": 1,
                "sets": [
                    { "set_number": 1, "duration_seconds": 12, "weight_kg": null, "rpe": 7 }
                ]
            }
        ]
    })
}

async fn create_sample_workout(app: &Router) -> serde_json::Value {
    let (status, body) = send_request_with_method(
        (*app).clone(),
        "/api/workouts",
        Method::POST,
        Some(sample_workout_body()),
        Some("test-api-key"),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "create failed: {body}");
    body
}

#[tokio::test]
async fn test_create_workout_with_nested_exercises_and_sets() {
    let app = common::test_app().await;

    let (status, body) = send_request_with_method(
        app,
        "/api/workouts",
        Method::POST,
        Some(sample_workout_body()),
        Some("test-api-key"),
    )
    .await;

    assert_eq!(status, StatusCode::CREATED);
    assert!(body["id"].is_number());
    assert_eq!(body["date"], "2026-08-23");
    assert_eq!(body["name"], "Pull day");
    let exercises = body["exercises"].as_array().unwrap();
    assert_eq!(exercises.len(), 2);
    assert_eq!(exercises[0]["exercise"]["name"], "Pull-up");
    assert_eq!(exercises[0]["sets"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn test_get_workout_embeds_exercises_and_sets_in_order() {
    let app = common::test_app().await;
    let created = create_sample_workout(&app).await;
    let id = created["id"].as_i64().unwrap();

    let (status, body) = send_request_with_method(
        app,
        &format!("/api/workouts/{id}"),
        Method::GET,
        None,
        Some("test-api-key"),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["id"], id);
    assert_eq!(body["date"], "2026-08-23");
    assert_eq!(body["name"], "Pull day");
    assert_eq!(body["notes"], "Felt strong");
    assert!(body.get("body_weight_kg").is_none());

    let exercises = body["exercises"].as_array().unwrap();
    assert_eq!(exercises.len(), 2);
    // order_index order
    assert_eq!(exercises[0]["exercise"]["name"], "Pull-up");
    assert_eq!(exercises[0]["exercise"]["category"], "calisthenics");
    assert_eq!(exercises[0]["order_index"], 0);
    assert_eq!(exercises[0]["notes"], "grip gave out");
    assert_eq!(exercises[1]["exercise"]["name"], "Planche Hold");

    let sets = exercises[0]["sets"].as_array().unwrap();
    assert_eq!(sets.len(), 2);
    assert_eq!(sets[0]["set_number"], 1);
    assert_eq!(sets[0]["reps"], 10);
    // added-weight convention: bodyweight pull-up stores weight_kg null
    assert_eq!(sets[0]["weight_kg"], serde_json::Value::Null);
    assert_eq!(sets[0]["rpe"], 8);
    assert_eq!(sets[1]["weight_kg"], 5.0);

    // timed hold: duration_seconds with reps null
    let hold_sets = exercises[1]["sets"].as_array().unwrap();
    assert_eq!(hold_sets[0]["duration_seconds"], 12);
    assert_eq!(hold_sets[0]["reps"], serde_json::Value::Null);
}

#[tokio::test]
async fn test_get_workout_404_for_unknown_id() {
    let app = common::test_app().await;

    let (status, _) = send_request_with_method(
        app,
        "/api/workouts/99999",
        Method::GET,
        None,
        Some("test-api-key"),
    )
    .await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_list_workouts_newest_first_with_filters() {
    let app = common::test_app().await;

    for (date, name) in [
        ("2026-08-01", "Week 1"),
        ("2026-08-15", "Week 3"),
        ("2026-08-23", "Week 4"),
    ] {
        let mut body = sample_workout_body();
        body["date"] = json!(date);
        body["name"] = json!(name);
        let (status, _) = send_request_with_method(
            app.clone(),
            "/api/workouts",
            Method::POST,
            Some(body),
            Some("test-api-key"),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
    }

    let (status, list) = send_request_with_method(
        app.clone(),
        "/api/workouts",
        Method::GET,
        None,
        Some("test-api-key"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let workouts = list.as_array().unwrap();
    assert_eq!(workouts.len(), 3);
    // newest-first
    assert_eq!(workouts[0]["name"], "Week 4");
    assert_eq!(workouts[2]["name"], "Week 1");
    // history list shape: id, date, name
    assert!(workouts[0]["id"].is_number());
    assert!(workouts[0]["date"].is_string());
    assert!(workouts[0]["exercises"].is_null());

    let (status, filtered) = send_request_with_method(
        app.clone(),
        "/api/workouts?from=2026-08-10&to=2026-08-20",
        Method::GET,
        None,
        Some("test-api-key"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let workouts = filtered.as_array().unwrap();
    assert_eq!(workouts.len(), 1);
    assert_eq!(workouts[0]["name"], "Week 3");

    let (status, limited) = send_request_with_method(
        app,
        "/api/workouts?limit=2",
        Method::GET,
        None,
        Some("test-api-key"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let workouts = limited.as_array().unwrap();
    assert_eq!(workouts.len(), 2);
    assert_eq!(workouts[0]["name"], "Week 4");
    assert_eq!(workouts[1]["name"], "Week 3");
}

#[tokio::test]
async fn test_patch_workout_updates_fields_and_replaces_sets() {
    let app = common::test_app().await;
    let created = create_sample_workout(&app).await;
    let id = created["id"].as_i64().unwrap();

    let patch = json!({
        "name": "Pull day (revised)",
        "notes": " shortened rest ",
        "exercises": [
            {
                "exercise_id": 11,
                "order_index": 0,
                "sets": [
                    { "set_number": 1, "reps": 12, "weight_kg": 2.5, "rpe": 9 }
                ]
            }
        ]
    });
    let (status, body) = send_request_with_method(
        app.clone(),
        &format!("/api/workouts/{id}"),
        Method::PATCH,
        Some(patch),
        Some("test-api-key"),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["name"], "Pull day (revised)");
    assert!(body.get("body_weight_kg").is_none());

    let (status, fetched) = send_request_with_method(
        app,
        &format!("/api/workouts/{id}"),
        Method::GET,
        None,
        Some("test-api-key"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let exercises = fetched["exercises"].as_array().unwrap();
    assert_eq!(exercises.len(), 1, "old nested exercises must be replaced");
    let sets = exercises[0]["sets"].as_array().unwrap();
    assert_eq!(sets.len(), 1);
    assert_eq!(sets[0]["reps"], 12);
    assert_eq!(sets[0]["weight_kg"], 2.5);
}

#[tokio::test]
async fn test_patch_workout_null_clears_optional_fields_absent_keeps() {
    let app = common::test_app().await;
    let created = create_sample_workout(&app).await;
    let id = created["id"].as_i64().unwrap();

    // explicit null clears the name; absent notes keep their value
    let (status, body) = send_request_with_method(
        app.clone(),
        &format!("/api/workouts/{id}"),
        Method::PATCH,
        Some(json!({ "name": null })),
        Some("test-api-key"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["name"], serde_json::Value::Null);
    assert_eq!(body["notes"], "Felt strong");
    assert!(body.get("body_weight_kg").is_none());

    // absent name keeps the cleared value
    let (status, body) = send_request_with_method(
        app.clone(),
        &format!("/api/workouts/{id}"),
        Method::PATCH,
        Some(json!({ "notes": "updated" })),
        Some("test-api-key"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["notes"], "updated");
    assert_eq!(body["name"], serde_json::Value::Null);

    // clearing persists
    let (status, fetched) = send_request_with_method(
        app,
        &format!("/api/workouts/{id}"),
        Method::GET,
        None,
        Some("test-api-key"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(fetched["name"], serde_json::Value::Null);
}

#[tokio::test]
async fn test_delete_workout_removes_children() {
    let app = common::test_app().await;
    let created = create_sample_workout(&app).await;
    let id = created["id"].as_i64().unwrap();

    let (status, _) = send_request_with_method(
        app.clone(),
        &format!("/api/workouts/{id}"),
        Method::DELETE,
        None,
        Some("test-api-key"),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, _) = send_request_with_method(
        app,
        &format!("/api/workouts/{id}"),
        Method::GET,
        None,
        Some("test-api-key"),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_create_workout_rolls_back_on_bad_exercise_reference() {
    let app = common::test_app().await;

    let mut body = sample_workout_body();
    // second entry references a non-existent exercise -> FK violation mid-transaction
    body["exercises"][1]["exercise_id"] = json!(99999);

    let (status, _) = send_request_with_method(
        app.clone(),
        "/api/workouts",
        Method::POST,
        Some(body),
        Some("test-api-key"),
    )
    .await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    // nothing may persist
    let (status, list) = send_request_with_method(
        app.clone(),
        "/api/workouts",
        Method::GET,
        None,
        Some("test-api-key"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(list.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_create_workout_rejects_set_without_reps_or_duration() {
    let app = common::test_app().await;

    let mut body = sample_workout_body();
    body["exercises"][0]["sets"][0] = json!({ "set_number": 1, "weight_kg": 5.0 });

    let (status, response) = send_request_with_method(
        app,
        "/api/workouts",
        Method::POST,
        Some(body),
        Some("test-api-key"),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(response["error"].as_str().unwrap().contains("reps"));
}

#[tokio::test]
async fn test_create_workout_rejects_rpe_out_of_range() {
    let app = common::test_app().await;

    for rpe in [0, 11] {
        let mut body = sample_workout_body();
        body["exercises"][0]["sets"][0]["rpe"] = json!(rpe);

        let (status, _) = send_request_with_method(
            app.clone(),
            "/api/workouts",
            Method::POST,
            Some(body),
            Some("test-api-key"),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "rpe {rpe}");
    }
}

#[tokio::test]
async fn test_create_workout_rejects_missing_date() {
    let app = common::test_app().await;

    let mut body = sample_workout_body();
    body.as_object_mut().unwrap().remove("date");

    let (status, _) = send_request_with_method(
        app,
        "/api/workouts",
        Method::POST,
        Some(body),
        Some("test-api-key"),
    )
    .await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_create_workout_rejects_non_iso_date() {
    let app = common::test_app().await;

    let mut body = sample_workout_body();
    body["date"] = json!("08/23/2026");

    let (status, response) = send_request_with_method(
        app.clone(),
        "/api/workouts",
        Method::POST,
        Some(body),
        Some("test-api-key"),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(response["error"].as_str().unwrap().contains("YYYY-MM-DD"));
}

#[tokio::test]
async fn test_create_workout_normalizes_unpadded_date() {
    let app = common::test_app().await;

    let mut body = sample_workout_body();
    body["date"] = json!("2026-8-3");

    let (status, response) = send_request_with_method(
        app,
        "/api/workouts",
        Method::POST,
        Some(body),
        Some("test-api-key"),
    )
    .await;

    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(response["date"], "2026-08-03");
}

#[tokio::test]
async fn test_delete_exercise_conflict_when_logged_in_workout() {
    let app = common::test_app().await;
    create_sample_workout(&app).await;

    // Pull-up is referenced by workout_exercises
    let (_, response) = send_request_with_method(
        app.clone(),
        "/api/exercises",
        Method::GET,
        None,
        Some("test-api-key"),
    )
    .await;
    let pull_up = response
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["name"] == "Pull-up")
        .unwrap();
    let id = pull_up["id"].as_i64().unwrap();

    let (status, body) = send_request_with_method(
        app,
        &format!("/api/exercises/{id}"),
        Method::DELETE,
        None,
        Some("test-api-key"),
    )
    .await;

    assert_eq!(status, StatusCode::CONFLICT);
    assert!(body["error"].as_str().is_some());
}

#[tokio::test]
async fn test_workouts_require_auth() {
    let app = common::test_app().await;

    let (status, _) = send_request_with_method(app, "/api/workouts", Method::GET, None, None).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}
