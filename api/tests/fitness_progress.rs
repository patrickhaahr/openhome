#[path = "common/memory_noop_db.rs"]
mod memory_noop_db;
#[path = "common/send_request_with_method.rs"]
mod send_request_with_method;
#[path = "common/test_app.rs"]
mod test_app;
#[path = "common/test_app_with_db.rs"]
mod test_app_with_db;
#[path = "common/test_app_with_pools.rs"]
mod test_app_with_pools;

use crate::send_request_with_method::send_request_with_method;
use crate::test_app::test_app;
use axum::Router;
use http::Method;
use http::StatusCode;
use serde_json::json;

const DAY_A: &str = "2026-09-01";
const DAY_B: &str = "2026-09-03";
const EPLEY_DIVISOR: f64 = 30.0;

struct WeightedSet {
    reps: u32,
    weight_kg: f64,
    rpe: u32,
}

const DAY_A_SETS: [WeightedSet; 2] = [
    WeightedSet {
        reps: 10,
        weight_kg: 60.0,
        rpe: 7,
    },
    WeightedSet {
        reps: 8,
        weight_kg: 65.0,
        rpe: 8,
    },
];
const BODYWEIGHT_REPS: u32 = 12;
const BODYWEIGHT_RPE: u32 = 9;
const HOLD_SECONDS: u32 = 30;
const HOLD_WEIGHT_KG: f64 = 2.5;

fn best_reps(sets: &[WeightedSet]) -> u32 {
    sets.iter().map(|set| set.reps).max().unwrap()
}

fn best_rpe(sets: &[WeightedSet]) -> u32 {
    sets.iter().map(|set| set.rpe).max().unwrap()
}

fn best_weight_kg(sets: &[WeightedSet]) -> f64 {
    sets.iter().map(|set| set.weight_kg).fold(0.0, f64::max)
}

fn volume_kg(sets: &[WeightedSet]) -> f64 {
    sets.iter()
        .map(|set| f64::from(set.reps) * set.weight_kg)
        .sum()
}

fn best_estimated_1rm_kg(sets: &[WeightedSet]) -> f64 {
    sets.iter()
        .map(|set| set.weight_kg * (1.0 + f64::from(set.reps) / EPLEY_DIVISOR))
        .fold(0.0, f64::max)
}

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
            DAY_A,
            json!(
                DAY_A_SETS
                    .iter()
                    .zip(1..)
                    .map(|(set, set_number)| json!({
                        "set_number": set_number,
                        "reps": set.reps,
                        "weight_kg": set.weight_kg,
                        "rpe": set.rpe,
                    }))
                    .collect::<Vec<_>>()
            ),
        ),
        (
            DAY_B,
            json!([
                { "set_number": 1, "reps": BODYWEIGHT_REPS, "weight_kg": null, "rpe": BODYWEIGHT_RPE },
                { "set_number": 1, "duration_seconds": HOLD_SECONDS, "weight_kg": HOLD_WEIGHT_KG }
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
    let app = test_app().await;
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
    assert_eq!(data[0]["date"], DAY_A);
    assert_eq!(data[1]["date"], DAY_B);

    assert_eq!(data[0]["best_reps"], best_reps(&DAY_A_SETS));
    approx(
        &data[0]["best_weight_kg"],
        best_weight_kg(&DAY_A_SETS),
        "day A best_weight",
    );
    approx(
        &data[0]["total_volume_kg"],
        volume_kg(&DAY_A_SETS),
        "day A volume",
    );
    assert_eq!(data[0]["best_rpe"], best_rpe(&DAY_A_SETS));
    approx(
        &data[0]["estimated_1rm_kg"],
        best_estimated_1rm_kg(&DAY_A_SETS),
        "day A epley",
    );

    assert_eq!(data[1]["best_reps"], BODYWEIGHT_REPS);
    approx(
        &data[1]["best_weight_kg"],
        HOLD_WEIGHT_KG,
        "day B best_weight",
    );
    approx(
        &data[1]["total_volume_kg"],
        f64::from(HOLD_SECONDS) * HOLD_WEIGHT_KG,
        "day B volume",
    );
    assert_eq!(data[1]["best_rpe"], BODYWEIGHT_RPE);
    assert_eq!(
        data[1]["estimated_1rm_kg"],
        serde_json::Value::Null,
        "no set with both reps and weight -> no 1RM estimate"
    );
}

#[tokio::test]
async fn test_progress_from_to_filters_bound_series() {
    let app = test_app().await;
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
    assert_eq!(data[0]["date"], DAY_B);
}

#[tokio::test]
async fn test_progress_empty_for_never_performed_exercise() {
    let app = test_app().await;
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
    let app = test_app().await;

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
