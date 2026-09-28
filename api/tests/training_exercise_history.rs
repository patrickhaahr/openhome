//! Exact Set history for one Exercise through the authenticated training API.

mod common;

use axum::Router;
use axum::http::{Method, StatusCode};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

const KEY: &str = "test-api-key";
const PULL_UP: i64 = 11;

async fn log(app: &Router, workout: Value) -> i64 {
    let (status, body) = common::send_request_with_method(
        app.clone(),
        "/api/workouts",
        Method::POST,
        Some(workout),
        Some(KEY),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body["id"].as_i64().unwrap()
}

async fn history(
    app: &Router,
    name: &str,
    from: &str,
    to: &str,
    key: Option<&str>,
) -> (StatusCode, Value) {
    common::send_request(
        app.clone(),
        &format!("/api/training/exercises/{name}/history/{from}/{to}"),
        key,
    )
    .await
}

#[tokio::test]
async fn returns_every_set_in_workout_and_logged_order_at_both_range_boundaries() {
    let app = common::test_app().await;
    let first = log(
        &app,
        json!({"date": "2026-06-01", "name": "Pull A", "notes": "fresh",
            "exercises": [{"exercise_id": PULL_UP, "order_index": 1, "notes": "wide grip",
                "sets": [
                    {"set_number": 2, "reps": 4, "weight_kg": 10.0, "rpe": 9, "notes": "hard"},
                    {"set_number": 1, "reps": 5, "weight_kg": null, "rpe": 8}
                ]},
                {"exercise_id": PULL_UP, "order_index": 0, "notes": "narrow grip",
                 "sets": []}]}),
    )
    .await;
    let second = log(
        &app,
        json!({"date": "2026-06-01", "name": "Pull B", "notes": null,
            "exercises": [{"exercise_id": PULL_UP, "order_index": 0, "notes": null,
                "sets": [{"set_number": 1, "duration_seconds": 20, "rpe": null}]}]}),
    )
    .await;
    let last = log(
        &app,
        json!({"date": "2026-08-29", "name": null, "notes": "last day",
            "exercises": [{"exercise_id": PULL_UP, "order_index": 0, "notes": null,
                "sets": [{"set_number": 1, "reps": 6, "weight_kg": 12.5, "rpe": 10}]}]}),
    )
    .await;
    log(
        &app,
        json!({"date": "2026-08-30", "exercises": [
            {"exercise_id": PULL_UP, "order_index": 0,
             "sets": [{"set_number": 1, "reps": 7}]}
        ]}),
    )
    .await;

    let (status, body) = history(&app, "pull-up", "2026-06-01", "2026-08-29", Some(KEY)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({
            "exercise": {"id": PULL_UP, "name": "Pull-up", "category": "calisthenics"},
            "from_day": "2026-06-01", "to_day": "2026-08-29",
            "time_zone": "Europe/Copenhagen",
            "workouts": [
                {"id": first, "date": "2026-06-01", "name": "Pull A", "notes": "fresh",
                 "entries": [{"notes": "narrow grip", "sets": []},
                    {"notes": "wide grip", "sets": [
                    {"set_number": 1, "reps": 5, "added_weight_kg": null,
                     "hold_duration_s": null, "rpe": 8, "notes": null},
                    {"set_number": 2, "reps": 4, "added_weight_kg": 10.0,
                     "hold_duration_s": null, "rpe": 9, "notes": "hard"}
                 ]}]},
                {"id": second, "date": "2026-06-01", "name": "Pull B", "notes": null,
                 "entries": [{"notes": null, "sets": [
                    {"set_number": 1, "reps": null, "added_weight_kg": null,
                     "hold_duration_s": 20, "rpe": null, "notes": null}
                 ]}]},
                {"id": last, "date": "2026-08-29", "name": null, "notes": "last day",
                 "entries": [{"notes": null, "sets": [
                    {"set_number": 1, "reps": 6, "added_weight_kg": 12.5,
                     "hold_duration_s": null, "rpe": 10, "notes": null}
                 ]}]}
            ]
        })
    );
}

#[tokio::test]
async fn rejects_oversized_history_instead_of_truncating_it() {
    let (app, state) = common::test_app_with_db().await;
    sqlx::query("INSERT INTO workouts (id, date) VALUES (1, '2026-09-01')")
        .execute(&state.db)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO workout_exercises (id, workout_id, exercise_id, order_index) \
         VALUES (1, 1, 11, 0)",
    )
    .execute(&state.db)
    .await
    .unwrap();
    sqlx::query(
        "WITH RECURSIVE numbers(n) AS (SELECT 1 UNION ALL SELECT n + 1 FROM numbers WHERE n < 501) \
         INSERT INTO sets (workout_exercise_id, set_number, reps) SELECT 1, n, 5 FROM numbers",
    )
    .execute(&state.db)
    .await
    .unwrap();

    let (status, body) = history(&app, "Pull-up", "2026-09-01", "2026-09-01", Some(KEY)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(body["error"].as_str().unwrap().contains("500"));
}

#[tokio::test]
async fn reports_empty_unknown_ambiguous_and_invalid_requests() {
    let (app, state) = common::test_app_with_db().await;
    let (status, body) = history(&app, "Pull-up", "2026-09-01", "2026-09-01", Some(KEY)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["workouts"], json!([]));
    assert_eq!(body["exercise"]["name"], "Pull-up");

    let (status, _) = history(&app, "Pull-up", "2026-09-01", "2026-09-01", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, body) = history(&app, "Unknown", "2026-09-01", "2026-09-01", Some(KEY)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(body["error"].as_str().unwrap().contains("Unknown"));

    sqlx::query("INSERT INTO exercises (name, category) VALUES ('PULL-UP', 'gym')")
        .execute(&state.db)
        .await
        .unwrap();
    let (status, body) = history(&app, "pull-up", "2026-09-01", "2026-09-01", Some(KEY)).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(body["error"].as_str().unwrap().contains("ambiguous"));

    for (from, to) in [
        ("2026-02-30", "2026-03-01"),
        ("2026-09-02", "2026-09-01"),
        ("2026-06-01", "2026-08-30"),
    ] {
        let (status, _) = history(&app, "Pull-up", from, to, Some(KEY)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{from}..{to}");
    }
}
