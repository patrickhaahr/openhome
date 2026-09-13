mod common;

use axum::Router;
use common::send_request_with_method;
use http::{Method, StatusCode};
use serde_json::json;

#[tokio::test]
async fn test_list_exercises_returns_seeded_catalog() {
    let app = common::test_app().await;

    let (status, response) = send_request_with_method(
        app,
        "/api/exercises",
        Method::GET,
        None,
        Some("test-api-key"),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let exercises = response.as_array().unwrap();
    assert!(exercises.len() >= 12);

    let names: Vec<&str> = exercises
        .iter()
        .filter_map(|e| e["name"].as_str())
        .collect();
    let expected: &[&str] = &[
        // Calisthenics
        "Planche Hold",
        "Planche Push-up",
        "Planche Press",
        "Front Lever Hold",
        "Front Lever Pull-up",
        "Front Lever Raise",
        "Front Lever Touch",
        "Handstand Push-up",
        "Handstand (timed)",
        "Ring Dip",
        "Pull-up",
        "Dip",
        // Gym
        "Converging Chest Press",
        "Seal Row",
        "T-bar Row",
        "Lat Pull-down",
        "Preacher Biceps Machine",
        "Side Delt Raise Cable",
        "Rear Delt Machine",
        "Triceps Pushdown",
        "Glute Bridge Machine",
    ];
    for name in expected {
        assert!(names.contains(name), "missing seed: {}", name);
    }

    let planche = exercises
        .iter()
        .find(|e| e["name"] == "Planche Hold")
        .unwrap();
    assert_eq!(planche["category"], "calisthenics");
    assert!(planche["muscle_group"].is_string());
    assert!(planche["equipment"].is_string());

    for e in exercises.iter().filter(|e| e["category"] == "gym") {
        let group = e["muscle_group"].as_str().unwrap();
        assert!(
            ["chest", "shoulders", "biceps", "triceps", "back", "glutes"].contains(&group),
            "unexpected muscle_group {} for gym seed",
            group
        );
    }
}

#[tokio::test]
async fn test_list_exercises_filter_by_category() {
    let app = common::test_app().await;

    let (status, response) = send_request_with_method(
        app,
        "/api/exercises?category=gym",
        Method::GET,
        None,
        Some("test-api-key"),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let exercises = response.as_array().unwrap();
    assert!(!exercises.is_empty());
    for e in exercises {
        assert_eq!(e["category"], "gym");
        assert_ne!(e["name"], "Pull-up");
    }
}

#[tokio::test]
async fn test_list_exercises_filter_by_muscle_group() {
    let app = common::test_app().await;

    let (status, response) = send_request_with_method(
        app,
        "/api/exercises?muscle_group=back",
        Method::GET,
        None,
        Some("test-api-key"),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let exercises = response.as_array().unwrap();
    assert!(!exercises.is_empty());
    for e in exercises {
        assert_eq!(e["muscle_group"], "back");
    }
}

#[tokio::test]
async fn test_list_exercises_filter_combined() {
    let app = common::test_app().await;

    let (status, response) = send_request_with_method(
        app,
        "/api/exercises?category=gym&muscle_group=chest",
        Method::GET,
        None,
        Some("test-api-key"),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    for e in response.as_array().unwrap() {
        assert_eq!(e["category"], "gym");
        assert_eq!(e["muscle_group"], "chest");
    }
}

#[tokio::test]
async fn test_get_exercise_by_id() {
    let app = common::test_app().await;

    let (_status, list) = send_request_with_method(
        app.clone(),
        "/api/exercises",
        Method::GET,
        None,
        Some("test-api-key"),
    )
    .await;
    let id = list[0]["id"].as_i64().unwrap();

    let (status, response) = send_request_with_method(
        app,
        &format!("/api/exercises/{}", id),
        Method::GET,
        None,
        Some("test-api-key"),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(response["id"], id);
    assert!(response["name"].is_string());
    assert!(response["category"].is_string());
}

#[tokio::test]
async fn test_get_exercise_404_for_unknown_id() {
    let app = common::test_app().await;

    let (status, _) = send_request_with_method(
        app,
        "/api/exercises/99999",
        Method::GET,
        None,
        Some("test-api-key"),
    )
    .await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_create_exercise_returns_201() {
    let app = common::test_app().await;

    let body = json!({
        "name": "Incline Dumbbell Press",
        "category": "gym",
        "muscle_group": "chest",
        "equipment": "dumbbell"
    });

    let (status, response) = send_request_with_method(
        app.clone(),
        "/api/exercises",
        Method::POST,
        Some(body),
        Some("test-api-key"),
    )
    .await;

    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(response["name"], "Incline Dumbbell Press");
    assert_eq!(response["category"], "gym");
    assert_eq!(response["muscle_group"], "chest");
    assert_eq!(response["equipment"], "dumbbell");
    assert!(response["id"].is_number());
}

#[tokio::test]
async fn test_create_exercise_allows_optional_fields_null() {
    let app = common::test_app().await;

    let body = json!({
        "name": "Mystery Movement",
        "category": "gym"
    });

    let (status, response) = send_request_with_method(
        app,
        "/api/exercises",
        Method::POST,
        Some(body),
        Some("test-api-key"),
    )
    .await;

    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(response["muscle_group"], serde_json::Value::Null);
    assert_eq!(response["equipment"], serde_json::Value::Null);
}

#[tokio::test]
async fn test_create_exercise_rejects_unknown_category() {
    let app = common::test_app().await;

    let body = json!({
        "name": "Bad Category Move",
        "category": "yoga"
    });

    let (status, response) = send_request_with_method(
        app,
        "/api/exercises",
        Method::POST,
        Some(body),
        Some("test-api-key"),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(response["error"].as_str().unwrap().contains("category"));
}

#[tokio::test]
async fn test_create_exercise_conflict_on_duplicate_name() {
    let app = common::test_app().await;

    let body = json!({
        "name": "Pull-up",
        "category": "calisthenics"
    });

    let (status, response) = send_request_with_method(
        app.clone(),
        "/api/exercises",
        Method::POST,
        Some(body),
        Some("test-api-key"),
    )
    .await;

    assert_eq!(status, StatusCode::CONFLICT);
    assert!(response["error"].as_str().unwrap().contains("Pull-up"));
}

#[tokio::test]
async fn test_patch_exercise_updates_fields() {
    let app = common::test_app().await;

    let body = json!({
        "name": "Temp Exercise",
        "category": "gym",
        "muscle_group": "chest",
        "equipment": "machine"
    });
    let (_status, created) = send_request_with_method(
        app.clone(),
        "/api/exercises",
        Method::POST,
        Some(body),
        Some("test-api-key"),
    )
    .await;
    let id = created["id"].as_i64().unwrap();

    let patch = json!({
        "muscle_group": "shoulders",
        "equipment": "cable"
    });
    let (status, response) = send_request_with_method(
        app.clone(),
        &format!("/api/exercises/{}", id),
        Method::PATCH,
        Some(patch),
        Some("test-api-key"),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(response["name"], "Temp Exercise");
    assert_eq!(response["muscle_group"], "shoulders");
    assert_eq!(response["equipment"], "cable");

    let (_status, fetched) = send_request_with_method(
        app,
        &format!("/api/exercises/{}", id),
        Method::GET,
        None,
        Some("test-api-key"),
    )
    .await;
    assert_eq!(fetched["muscle_group"], "shoulders");
}

#[tokio::test]
async fn test_patch_exercise_null_clears_optional_fields_absent_keeps() {
    let app = common::test_app().await;

    let body = json!({
        "name": "Clearable Move",
        "category": "gym",
        "muscle_group": "chest",
        "equipment": "machine"
    });
    let (_status, created) = send_request_with_method(
        app.clone(),
        "/api/exercises",
        Method::POST,
        Some(body),
        Some("test-api-key"),
    )
    .await;
    let id = created["id"].as_i64().unwrap();

    // explicit null clears; absent equipment keeps its value
    let (status, response) = send_request_with_method(
        app.clone(),
        &format!("/api/exercises/{}", id),
        Method::PATCH,
        Some(json!({ "muscle_group": null })),
        Some("test-api-key"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(response["muscle_group"], serde_json::Value::Null);
    assert_eq!(response["equipment"], "machine");

    // absent muscle_group keeps the cleared value
    let (status, response) = send_request_with_method(
        app,
        &format!("/api/exercises/{}", id),
        Method::PATCH,
        Some(json!({ "name": "Clearable Move II" })),
        Some("test-api-key"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(response["name"], "Clearable Move II");
    assert_eq!(response["muscle_group"], serde_json::Value::Null);
}

#[tokio::test]
async fn test_patch_exercise_404_for_unknown_id() {
    let app = common::test_app().await;

    let patch = json!({ "muscle_group": "back" });
    let (status, _) = send_request_with_method(
        app,
        "/api/exercises/99999",
        Method::PATCH,
        Some(patch),
        Some("test-api-key"),
    )
    .await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_patch_exercise_conflict_on_duplicate_name() {
    let app = common::test_app().await;

    let body = json!({
        "name": "Patch Conflict A",
        "category": "gym"
    });
    let (_status, created) = send_request_with_method(
        app.clone(),
        "/api/exercises",
        Method::POST,
        Some(body),
        Some("test-api-key"),
    )
    .await;
    let id = created["id"].as_i64().unwrap();

    let patch = json!({ "name": "Pull-up" });
    let (status, _) = send_request_with_method(
        app,
        &format!("/api/exercises/{}", id),
        Method::PATCH,
        Some(patch),
        Some("test-api-key"),
    )
    .await;

    assert_eq!(status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn test_delete_exercise() {
    let app = common::test_app().await;

    let body = json!({
        "name": "Delete Me Press",
        "category": "gym"
    });
    let (_status, created) = send_request_with_method(
        app.clone(),
        "/api/exercises",
        Method::POST,
        Some(body),
        Some("test-api-key"),
    )
    .await;
    let id = created["id"].as_i64().unwrap();

    let (status, _) = send_request_with_method(
        app.clone(),
        &format!("/api/exercises/{}", id),
        Method::DELETE,
        None,
        Some("test-api-key"),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, _) = send_request_with_method(
        app,
        &format!("/api/exercises/{}", id),
        Method::GET,
        None,
        Some("test-api-key"),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_delete_exercise_404_for_unknown_id() {
    let app = common::test_app().await;

    let (status, _) = send_request_with_method(
        app,
        "/api/exercises/99999",
        Method::DELETE,
        None,
        Some("test-api-key"),
    )
    .await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_exercises_require_auth() {
    let app = common::test_app().await;

    let (status, _) =
        send_request_with_method(app, "/api/exercises", Method::GET, None, None).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// ---------------------------------------------------------------------------
// Workout logging (#7)
// ---------------------------------------------------------------------------

fn sample_workout_body() -> serde_json::Value {
    json!({
        "date": "2026-08-23",
        "name": "Pull day",
        "notes": "Felt strong",
        "body_weight_kg": 74.5,
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
    assert_eq!(status, StatusCode::CREATED, "create failed: {}", body);
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
        &format!("/api/workouts/{}", id),
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
    assert_eq!(body["body_weight_kg"], 74.5);

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
        "body_weight_kg": 75.0,
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
        &format!("/api/workouts/{}", id),
        Method::PATCH,
        Some(patch),
        Some("test-api-key"),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["name"], "Pull day (revised)");
    assert_eq!(body["body_weight_kg"], 75.0);

    let (status, fetched) = send_request_with_method(
        app,
        &format!("/api/workouts/{}", id),
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

    // explicit null clears the name; absent notes/body_weight keep values
    let (status, body) = send_request_with_method(
        app.clone(),
        &format!("/api/workouts/{}", id),
        Method::PATCH,
        Some(json!({ "name": null })),
        Some("test-api-key"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["name"], serde_json::Value::Null);
    assert_eq!(body["notes"], "Felt strong");
    assert_eq!(body["body_weight_kg"], 74.5);

    // absent name keeps the cleared value
    let (status, body) = send_request_with_method(
        app.clone(),
        &format!("/api/workouts/{}", id),
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
        &format!("/api/workouts/{}", id),
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
        &format!("/api/workouts/{}", id),
        Method::DELETE,
        None,
        Some("test-api-key"),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, _) = send_request_with_method(
        app,
        &format!("/api/workouts/{}", id),
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
        assert_eq!(status, StatusCode::BAD_REQUEST, "rpe {}", rpe);
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
    let (status, response) = send_request_with_method(
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
        &format!("/api/exercises/{}", id),
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

// ---------------------------------------------------------------------------
// Progress (#8)
// ---------------------------------------------------------------------------

/// Fixture: a dedicated exercise logged on two dates.
/// Day A (2026-09-01): reps 10 @ 60 kg (rpe 7), reps 8 @ 65 kg (rpe 8).
///   best_reps 10, best_weight 65, volume 600+520=1120, best_rpe 8,
///   epley = max(60*1.333, 65*1.2667) = 82.333.
/// Day B (2026-09-03): reps 12 bodyweight (rpe 9), timed hold 30 s @ 2.5 kg.
///   best_reps 12, best_weight 2.5, volume 0+75=75, best_rpe 9, epley null
///   (no set with both reps and weight).
async fn create_progress_fixture(app: &Router) -> i64 {
    let (status, exercise) = send_request_with_method(
        (*app).clone(),
        "/api/exercises",
        Method::POST,
        Some(json!({ "name": "Progress Bench", "category": "gym" })),
        Some("test-api-key"),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "exercise create: {}", exercise);
    let id = exercise["id"].as_i64().unwrap();

    let workouts = [
        (
            "2026-09-01",
            json!([
                { "set_number": 1, "reps": 10, "weight_kg": 60.0, "rpe": 7 },
                { "set_number": 2, "reps": 8, "weight_kg": 65.0, "rpe": 8 }
            ]),
        ),
        (
            "2026-09-03",
            json!([
                { "set_number": 1, "reps": 12, "weight_kg": null, "rpe": 9 },
                { "set_number": 1, "duration_seconds": 30, "weight_kg": 2.5 }
            ]),
        ),
    ];
    for (date, sets) in workouts {
        let body = json!({
            "date": date,
            "exercises": [
                { "exercise_id": id, "order_index": 0, "sets": sets }
            ]
        });
        let (status, resp) = send_request_with_method(
            (*app).clone(),
            "/api/workouts",
            Method::POST,
            Some(body),
            Some("test-api-key"),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "workout {}: {}", date, resp);
    }
    id
}

fn approx(actual: serde_json::Value, expected: f64, what: &str) {
    let value = actual
        .as_f64()
        .unwrap_or_else(|| panic!("{} not a number", what));
    assert!(
        (value - expected).abs() < 0.01,
        "{}: expected ~{}, got {}",
        what,
        expected,
        value
    );
}

#[tokio::test]
async fn test_progress_returns_per_date_aggregates_with_epley() {
    let app = common::test_app().await;
    let id = create_progress_fixture(&app).await;

    let (status, body) = send_request_with_method(
        app,
        &format!("/api/exercises/{}/progress", id),
        Method::GET,
        None,
        Some("test-api-key"),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["exercise"]["id"], id);
    assert_eq!(body["exercise"]["name"], "Progress Bench");

    let data = body["data"].as_array().unwrap();
    assert_eq!(data.len(), 2);
    // chronological order
    assert_eq!(data[0]["date"], "2026-09-01");
    assert_eq!(data[1]["date"], "2026-09-03");

    // Day A: observed aggregates + Epley from the best set
    assert_eq!(data[0]["best_reps"], 10);
    approx(data[0]["best_weight_kg"].clone(), 65.0, "day A best_weight");
    approx(data[0]["total_volume_kg"].clone(), 1120.0, "day A volume");
    assert_eq!(data[0]["best_rpe"], 8);
    approx(
        data[0]["estimated_1rm_kg"].clone(),
        65.0 * (1.0 + 8.0 / 30.0),
        "day A epley",
    );

    // Day B: bodyweight set contributes zero volume; no set has reps AND weight
    assert_eq!(data[1]["best_reps"], 12);
    approx(data[1]["best_weight_kg"].clone(), 2.5, "day B best_weight");
    approx(data[1]["total_volume_kg"].clone(), 75.0, "day B volume");
    assert_eq!(data[1]["best_rpe"], 9);
    assert_eq!(
        data[1]["estimated_1rm_kg"],
        serde_json::Value::Null,
        "no set with both reps and weight -> no 1RM estimate"
    );
}

#[tokio::test]
async fn test_progress_from_to_filters_bound_series() {
    let app = common::test_app().await;
    let id = create_progress_fixture(&app).await;

    let (status, body) = send_request_with_method(
        app.clone(),
        &format!(
            "/api/exercises/{}/progress?from=2026-09-02&to=2026-09-02",
            id
        ),
        Method::GET,
        None,
        Some("test-api-key"),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let data = body["data"].as_array().unwrap();
    assert!(data.is_empty(), "from/to must exclude out-of-range dates");

    let (status, body) = send_request_with_method(
        app,
        &format!("/api/exercises/{}/progress?from=2026-09-02", id),
        Method::GET,
        None,
        Some("test-api-key"),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let data = body["data"].as_array().unwrap();
    assert_eq!(data.len(), 1);
    assert_eq!(data[0]["date"], "2026-09-03");
}

#[tokio::test]
async fn test_progress_empty_for_never_performed_exercise() {
    let app = common::test_app().await;
    let id = create_progress_fixture(&app).await;

    let (status, body) = send_request_with_method(
        app.clone(),
        "/api/exercises",
        Method::GET,
        None,
        Some("test-api-key"),
    )
    .await;
    let other = body
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["name"] == "Pull-up")
        .unwrap();
    let other_id = other["id"].as_i64().unwrap();

    let (status, response) = send_request_with_method(
        app,
        &format!("/api/exercises/{}/progress", other_id),
        Method::GET,
        None,
        Some("test-api-key"),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(response["data"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_progress_404_for_unknown_exercise() {
    let app = common::test_app().await;

    let (status, _) = send_request_with_method(
        app,
        "/api/exercises/99999/progress",
        Method::GET,
        None,
        Some("test-api-key"),
    )
    .await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ---------------------------------------------------------------------------
// Body weight (#8)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_body_weight_record_and_list_sorted_by_date() {
    let app = common::test_app().await;

    for (date, weight) in [
        ("2026-09-05", 75.5),
        ("2026-09-01", 76.0),
        ("2026-09-03", 75.2),
    ] {
        let (status, body) = send_request_with_method(
            app.clone(),
            "/api/body_weight",
            Method::POST,
            Some(json!({ "date": date, "weight_kg": weight })),
            Some("test-api-key"),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "post {}: {}", date, body);
        assert_eq!(body["date"], date);
        assert_eq!(body["weight_kg"], weight);
        assert!(body["id"].is_number());
    }

    let (status, list) = send_request_with_method(
        app.clone(),
        "/api/body_weight",
        Method::GET,
        None,
        Some("test-api-key"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let entries = list.as_array().unwrap();
    assert_eq!(entries.len(), 3);
    // chronological: one row per date, sorted by date
    assert_eq!(entries[0]["date"], "2026-09-01");
    assert_eq!(entries[1]["date"], "2026-09-03");
    assert_eq!(entries[2]["date"], "2026-09-05");
    assert_eq!(entries[0]["weight_kg"], 76.0);

    let (status, filtered) = send_request_with_method(
        app,
        "/api/body_weight?from=2026-09-02&to=2026-09-04",
        Method::GET,
        None,
        Some("test-api-key"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let entries = filtered.as_array().unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["date"], "2026-09-03");
}

#[tokio::test]
async fn test_body_weight_ships_empty_and_conflicts_on_duplicate_date() {
    let app = common::test_app().await;

    let (status, list) = send_request_with_method(
        app.clone(),
        "/api/body_weight",
        Method::GET,
        None,
        Some("test-api-key"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(list.as_array().unwrap().is_empty());

    let (status, _) = send_request_with_method(
        app.clone(),
        "/api/body_weight",
        Method::POST,
        Some(json!({ "date": "2026-09-05", "weight_kg": 75.5 })),
        Some("test-api-key"),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    let (status, response) = send_request_with_method(
        app,
        "/api/body_weight",
        Method::POST,
        Some(json!({ "date": "2026-09-05", "weight_kg": 74.0 })),
        Some("test-api-key"),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(response["error"].as_str().unwrap().contains("2026-09-05"));
}

#[tokio::test]
async fn test_body_weight_rejects_bad_input() {
    let app = common::test_app().await;

    let (status, response) = send_request_with_method(
        app.clone(),
        "/api/body_weight",
        Method::POST,
        Some(json!({ "date": "09/05/2026", "weight_kg": 75.0 })),
        Some("test-api-key"),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(response["error"].as_str().unwrap().contains("YYYY-MM-DD"));

    let (status, _) = send_request_with_method(
        app,
        "/api/body_weight",
        Method::POST,
        Some(json!({ "date": "2026-09-05" })),
        Some("test-api-key"),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_body_weight_requires_auth() {
    let app = common::test_app().await;

    let (status, _) =
        send_request_with_method(app, "/api/body_weight", Method::GET, None, None).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

// ---------------------------------------------------------------------------
// Profile (#8)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_profile_get_not_configured_then_patch_sets_fields() {
    let app = common::test_app().await;

    let (status, body) = send_request_with_method(
        app.clone(),
        "/api/profile",
        Method::GET,
        None,
        Some("test-api-key"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["height_cm"], serde_json::Value::Null);
    assert_eq!(body["sex"], serde_json::Value::Null);

    let (status, body) = send_request_with_method(
        app.clone(),
        "/api/profile",
        Method::PATCH,
        Some(json!({ "height_cm": 180.5, "sex": "male" })),
        Some("test-api-key"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["height_cm"], 180.5);
    assert_eq!(body["sex"], "male");

    let (status, fetched) =
        send_request_with_method(app, "/api/profile", Method::GET, None, Some("test-api-key"))
            .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(fetched["height_cm"], 180.5);
    assert_eq!(fetched["sex"], "male");
}

#[tokio::test]
async fn test_profile_patch_keeps_absent_fields() {
    let app = common::test_app().await;

    let (status, _) = send_request_with_method(
        app.clone(),
        "/api/profile",
        Method::PATCH,
        Some(json!({ "height_cm": 180.5, "sex": "male" })),
        Some("test-api-key"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = send_request_with_method(
        app,
        "/api/profile",
        Method::PATCH,
        Some(json!({ "height_cm": 181.0 })),
        Some("test-api-key"),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["height_cm"], 181.0);
    assert_eq!(body["sex"], "male", "absent field must keep current value");
}

#[tokio::test]
async fn test_profile_patch_null_clears_fields() {
    let app = common::test_app().await;

    let (status, _) = send_request_with_method(
        app.clone(),
        "/api/profile",
        Method::PATCH,
        Some(json!({ "height_cm": 180.5, "sex": "male" })),
        Some("test-api-key"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // explicit null clears sex; absent height_cm keeps its value
    let (status, body) = send_request_with_method(
        app,
        "/api/profile",
        Method::PATCH,
        Some(json!({ "sex": null })),
        Some("test-api-key"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["sex"], serde_json::Value::Null);
    assert_eq!(body["height_cm"], 180.5);
}

#[tokio::test]
async fn test_profile_requires_auth() {
    let app = common::test_app().await;

    let (status, _) = send_request_with_method(app, "/api/profile", Method::GET, None, None).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}
