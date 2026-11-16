//! Workouts on a Training Day: logged and NOOP Workouts merged on one day.

//! Workouts on a Training Day (`GET /api/training/days/{day}/workouts`): Workouts logged through
//! the fitness API beside NOOP Workouts pushed through the real push endpoint.

mod common;
mod training_workouts_support;

use axum::http::StatusCode;
use common::mirror::Mirror;
use common::mirror::noop_workout;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use training_workouts_support::{
    API_KEY, COMPUTED, IMPORTED, PULL_UP, REAL_PUSH_AT, SOURCE, app, log_workout, mirror_with,
    real_noop_workout, workouts, workouts_on,
};

const RING_DIP: i64 = 10;

const PLANCHE_HOLD: i64 = 1;

/// A NOOP Workout as the API reports it: every metric null except `fields`.
fn reported(
    start: &str,
    end: &str,
    sport: &str,
    origin: &str,
    source: &str,
    fields: Value,
) -> Value {
    let mut workout = json!({
        "start": start, "end": end, "sport": sport, "origin": origin, "duration_min": null,
        "avg_hr_bpm": null, "max_hr_bpm": null, "strain_score": null, "energy_kcal": null,
        "distance_m": null, "steps": null, "notes": null, "source": source,
    });
    let Value::Object(fields) = fields else {
        panic!("fields must be a JSON object");
    };
    for (name, value) in fields {
        assert!(workout.get(&name).is_some(), "unknown field {name}");
        workout[&name] = value;
    }
    workout
}

/// `expected` after the JSON text round trip the response body takes: `serde_json`'s default float
/// parser may land one ulp away from the exact quotient the test computes.
fn as_parsed(expected: &Value) -> Value {
    serde_json::from_str(&expected.to_string()).unwrap()
}

#[tokio::test]
async fn requires_the_api_key() {
    let app = app(&Mirror::empty().await).await;
    for api_key in [None, Some("wrong-key")] {
        let (status, body) = workouts(&app, "2026-09-26", api_key).await;

        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(
            body,
            json!({"error": "Missing or invalid API key", "status": 401})
        );
    }
}

#[tokio::test]
async fn rejects_days_that_are_not_canonical_calendar_dates() {
    let app = app(&Mirror::empty().await).await;
    for day in ["2026-9-26", "2026-02-30", "20260926", "today"] {
        let (status, body) = workouts(&app, day, Some(API_KEY)).await;

        assert_eq!(status, StatusCode::BAD_REQUEST, "{day}");
        assert_eq!(
            body,
            json!({
                "error": format!(
                    "Invalid day '{day}': must be a Europe/Copenhagen calendar date written \
                     YYYY-MM-DD"
                ),
                "status": 400,
            })
        );
    }
}

#[tokio::test]
async fn shows_the_overnight_noop_workout_on_its_local_start_day() {
    let mirror = mirror_with(&[real_noop_workout()], &[], REAL_PUSH_AT).await;
    let app = app(&mirror).await;
    // The session began on the evening of the 25th in the training log's reckoning.
    let evening = log_workout(
        &app,
        json!({
            "date": "2026-09-25", "name": "Pull", "notes": null,
            "exercises": [{
                "exercise_id": PULL_UP, "order_index": 0, "notes": null,
                "sets": [{"set_number": 1, "reps": 5, "weight_kg": 12.5, "rpe": 10,
                          "notes": "+2 partials"}],
            }],
        }),
    )
    .await;

    let body = workouts_on(&app, "2026-09-26").await;
    assert_eq!(
        body,
        as_parsed(&json!({
            "day": "2026-09-26",
            "time_zone": "Europe/Copenhagen",
            "day_start": "2026-09-26T00:00:00+02:00",
            "day_end": "2026-09-27T00:00:00+02:00",
            "workouts": [],
            "noop_workouts": [reported(
                "2026-09-26T02:12:38+02:00",
                "2026-09-26T03:30:11+02:00",
                "Calisthenics",
                "manual",
                IMPORTED,
                json!({
                    "duration_min": 4653.247 / 60.0, "avg_hr_bpm": 81.0, "max_hr_bpm": 134.0,
                    "strain_score": 24.51, "energy_kcal": 400.8306717632,
                }),
            )],
            "noop": {
                "installation_id": SOURCE,
                "imported_device_id": IMPORTED,
                "computed_device_id": COMPUTED,
                "last_push_at": "2026-09-26T03:51:38+02:00",
                // The push arrived during the 26th, so later sessions that day may still come.
                "freshness": "partial",
                "coverage": {"workouts": "covered"},
            },
        }))
    );

    // The logged Workout keeps its own date; nothing moves it next to the NOOP Workout.
    let previous = workouts_on(&app, "2026-09-25").await;
    assert_eq!(previous["workouts"][0]["id"], evening);
    assert_eq!(previous["noop_workouts"], json!([]));
    assert_eq!(previous["noop"]["freshness"], "confirmed");
}

#[tokio::test]
async fn keeps_multiple_workouts_on_both_sides_distinct() {
    let mirror = mirror_with(
        &[
            noop_workout(
                1_790_438_400,
                1_790_442_600,
                "Calisthenics",
                "manual",
                json!({"durationS": 4200.0, "avgHr": 96.0, "maxHr": 151.0, "strain": 11.2}),
            ),
            noop_workout(
                1_790_398_800,
                1_790_401_500,
                "Running",
                "manual",
                json!({"durationS": 2700.0, "distanceM": 7400.0, "steps": 6900,
                       "notes": "easy", "zonesJSON": "[10,20,40,20,10]",
                       "routePolyline": "abc"}),
            ),
        ],
        // A bout NOOP detected at noon, away from both logged sessions.
        &[noop_workout(
            1_790_416_800,
            1_790_420_400,
            "detected",
            "my-whoop-noop",
            json!({"avgHr": 110.0}),
        )],
        "2026-09-27T01:00:00.000Z",
    )
    .await;
    let app = app(&mirror).await;
    let morning = log_workout(
        &app,
        json!({
            "date": "2026-09-26", "name": "Push", "notes": "felt strong",
            "exercises": [
                {
                    "exercise_id": PLANCHE_HOLD, "order_index": 1, "notes": "tuck",
                    "sets": [
                        {"set_number": 2, "duration_seconds": 12, "rpe": 9},
                        {"set_number": 1, "duration_seconds": 15, "weight_kg": null},
                    ],
                },
                {
                    "exercise_id": RING_DIP, "order_index": 0, "notes": null,
                    "sets": [{"set_number": 1, "reps": 8, "weight_kg": 20.0, "rpe": 8,
                              "notes": "paused"}],
                },
            ],
        }),
    )
    .await;
    let evening = log_workout(
        &app,
        json!({
            "date": "2026-09-26", "name": null, "notes": null,
            "exercises": [{
                "exercise_id": PULL_UP, "order_index": 0, "notes": null,
                "sets": [{"set_number": 1, "reps": 10}],
            }],
        }),
    )
    .await;
    // An empty Workout is still a logged Workout.
    let empty = log_workout(
        &app,
        json!({"date": "2026-09-26", "name": "Mobility", "notes": null, "exercises": []}),
    )
    .await;

    let body = workouts_on(&app, "2026-09-26").await;
    assert_eq!(
        body["workouts"],
        json!([
            {
                "id": morning, "name": "Push", "notes": "felt strong",
                "exercises": [
                    {
                        "exercise_id": RING_DIP, "exercise": "Ring Dip",
                        "category": "calisthenics", "notes": null,
                        "sets": [{"set_number": 1, "reps": 8, "added_weight_kg": 20.0,
                                  "hold_duration_s": null, "rpe": 8, "notes": "paused"}],
                    },
                    {
                        "exercise_id": PLANCHE_HOLD, "exercise": "Planche Hold",
                        "category": "calisthenics", "notes": "tuck",
                        "sets": [
                            {"set_number": 1, "reps": null, "added_weight_kg": null,
                             "hold_duration_s": 15, "rpe": null, "notes": null},
                            {"set_number": 2, "reps": null, "added_weight_kg": null,
                             "hold_duration_s": 12, "rpe": 9, "notes": null},
                        ],
                    },
                ],
            },
            {
                "id": evening, "name": null, "notes": null,
                "exercises": [{
                    "exercise_id": PULL_UP, "exercise": "Pull-up", "category": "calisthenics",
                    "notes": null,
                    "sets": [{"set_number": 1, "reps": 10, "added_weight_kg": null,
                              "hold_duration_s": null, "rpe": null, "notes": null}],
                }],
            },
            {"id": empty, "name": "Mobility", "notes": null, "exercises": []},
        ])
    );
    assert_eq!(
        body["noop_workouts"],
        json!([
            reported(
                "2026-09-26T07:00:00+02:00",
                "2026-09-26T07:45:00+02:00",
                "Running",
                "manual",
                IMPORTED,
                json!({"duration_min": 45.0, "distance_m": 7400.0, "steps": 6900,
                       "notes": "easy"}),
            ),
            reported(
                "2026-09-26T12:00:00+02:00",
                "2026-09-26T13:00:00+02:00",
                "detected",
                "detected",
                COMPUTED,
                json!({"avg_hr_bpm": 110.0}),
            ),
            reported(
                "2026-09-26T18:00:00+02:00",
                "2026-09-26T19:10:00+02:00",
                "Calisthenics",
                "manual",
                IMPORTED,
                json!({"duration_min": 70.0, "avg_hr_bpm": 96.0, "max_hr_bpm": 151.0,
                       "strain_score": 11.2}),
            ),
        ])
    );
    assert_eq!(body["noop"]["freshness"], "confirmed");
}

#[tokio::test]
async fn resolves_one_activity_stored_twice_like_noop() {
    let manual = noop_workout(
        1_790_438_400,
        1_790_442_600,
        "Calisthenics",
        "manual",
        json!({"avgHr": 96.0}),
    );
    let mirror = mirror_with(
        std::slice::from_ref(&manual),
        &[
            // The same row in the computed namespace, and the detector's wider shadow of it.
            manual.clone(),
            noop_workout(
                1_790_437_800,
                1_790_443_200,
                "detected",
                "my-whoop-noop",
                json!({"avgHr": 90.0, "maxHr": 150.0, "strain": 13.0}),
            ),
        ],
        REAL_PUSH_AT,
    )
    .await;

    let body = workouts_on(&app(&mirror).await, "2026-09-26").await;
    assert_eq!(
        body["noop_workouts"],
        json!([reported(
            "2026-09-26T18:00:00+02:00",
            "2026-09-26T19:10:00+02:00",
            "Calisthenics",
            "manual",
            IMPORTED,
            json!({"avg_hr_bpm": 96.0}),
        )])
    );
}
