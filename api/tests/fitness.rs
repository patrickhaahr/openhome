mod common;

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
