//! Exercise progress aggregates.

mod common;

use axum::Router;
use common::send_request_with_method;
use http::Method;
use http::StatusCode;
use serde_json::json;

// ---------------------------------------------------------------------------
// Progress (#8)
// ---------------------------------------------------------------------------

/// Fixture: a dedicated exercise logged on two dates.
/// Day A (2026-09-01): reps 10 @ 60 kg (rpe 7), reps 8 @ 65 kg (rpe 8).
///   `best_reps` 10, `best_weight` 65, volume 600+520=1120, `best_rpe` 8,
///   epley = max(60*1.333, 65*1.2667) = 82.333.
/// Day B (2026-09-03): reps 12 bodyweight (rpe 9), timed hold 30 s @ 2.5 kg.
///   `best_reps` 12, `best_weight` 2.5, volume 0+75=75, `best_rpe` 9, epley null
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
    assert_eq!(status, StatusCode::CREATED, "exercise create: {exercise}");
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
        assert_eq!(status, StatusCode::CREATED, "workout {date}: {resp}");
    }
    id
}

fn approx(actual: &serde_json::Value, expected: f64, what: &str) {
    let value = actual
        .as_f64()
        .unwrap_or_else(|| panic!("{what} not a number"));
    assert!(
        (value - expected).abs() < 0.01,
        "{what}: expected ~{expected}, got {value}"
    );
}

#[tokio::test]
async fn test_progress_returns_per_date_aggregates_with_epley() {
    let app = common::test_app().await;
    let id = create_progress_fixture(&app).await;

    let (status, body) = send_request_with_method(
        app,
        &format!("/api/exercises/{id}/progress"),
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
    approx(&data[0]["best_weight_kg"], 65.0, "day A best_weight");
    approx(&data[0]["total_volume_kg"], 1120.0, "day A volume");
    assert_eq!(data[0]["best_rpe"], 8);
    approx(
        &data[0]["estimated_1rm_kg"],
        65.0 * (1.0 + 8.0 / 30.0),
        "day A epley",
    );

    // Day B: bodyweight set contributes zero volume; no set has reps AND weight
    assert_eq!(data[1]["best_reps"], 12);
    approx(&data[1]["best_weight_kg"], 2.5, "day B best_weight");
    approx(&data[1]["total_volume_kg"], 75.0, "day B volume");
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
        &format!("/api/exercises/{id}/progress?from=2026-09-02&to=2026-09-02"),
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
        &format!("/api/exercises/{id}/progress?from=2026-09-02"),
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
    create_progress_fixture(&app).await;

    let (_, body) = send_request_with_method(
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
        &format!("/api/exercises/{other_id}/progress"),
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
