//! Training Context: logged training, body weight and recovery aligned by day.

//! Training Context (`GET /api/training/context/{from_day}/{to_day}`): Workouts and Body Weight
//! logged through the fitness API beside NOOP rows pushed through the real push endpoint, on one
//! Copenhagen date axis.

mod common;
mod training_context_support;

use axum::Router;
use axum::http::Method;
use axum::http::StatusCode;
use chrono::Duration;
use common::mirror::Mirror;
use common::mirror::Window;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use training_context_support::{
    API_KEY, COMPUTED, IMPORTED, SOURCE, at, context_ok, daily, day, mirror, noop_workout, sync,
};

/// Seeded exercise library ids.
const PULL_UP: i64 = 11;

const RING_DIP: i64 = 10;

const PLANCHE_HOLD: i64 = 1;

async fn post(app: &Router, uri: &str, body: Value) -> Value {
    let (status, body) =
        common::send_request_with_method(app.clone(), uri, Method::POST, Some(body), Some(API_KEY))
            .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body
}

async fn log_workout(app: &Router, workout: Value) -> i64 {
    post(app, "/api/workouts", workout).await["id"]
        .as_i64()
        .unwrap()
}

fn measurement(value: Option<f64>, unit: &str, source: Option<&str>) -> Value {
    json!({"value": value, "unit": unit, "source": source})
}

#[tokio::test]
async fn aligns_logged_training_body_weight_and_recovery_by_day() {
    let overnight = at("2026-09-30", Duration::minutes(30));
    let mirror = mirror(
        "2026-09-27",
        "2026-10-02",
        &[
            daily(
                "2026-09-28",
                json!({"totalSleepMin": 420.0, "restingHr": 50.0}),
            ),
            // A bare imported sleep total that NOOP's scored computed night replaces.
            daily("2026-09-30", json!({"totalSleepMin": 300.0})),
        ],
        &[
            daily(
                "2026-09-28",
                json!({"totalSleepMin": 410.0, "restingHr": 47.0, "avgHrv": 62.0}),
            ),
            daily(
                "2026-09-30",
                json!({"totalSleepMin": 330.0, "efficiency": 0.9}),
            ),
        ],
        &[noop_workout(
            overnight,
            overnight + 3600,
            "Calisthenics",
            "manual",
            json!({"durationS": 3600.0, "avgHr": 90.0, "maxHr": 140.0, "strain": 12.0,
                   "energyKcal": 300.0, "zonesJSON": "[1,2,3,4,5]", "routePolyline": "abc"}),
        )],
        "2026-10-02T02:00:00.000Z",
    )
    .await;
    let app = common::test_app_with_noop_db(mirror.db).await;
    post(
        &app,
        "/api/body_weight",
        json!({"date": "2026-09-28", "weight_kg": 80.0}),
    )
    .await;
    post(
        &app,
        "/api/body_weight",
        json!({"date": "2026-10-01", "weight_kg": 79.5}),
    )
    .await;
    let pull = log_workout(
        &app,
        json!({
            "date": "2026-09-28", "name": "Pull", "notes": "long notes stay in the day read",
            "exercises": [
                {
                    "exercise_id": PLANCHE_HOLD, "order_index": 1, "notes": null,
                    "sets": [
                        {"set_number": 1, "duration_seconds": 15},
                        {"set_number": 2, "duration_seconds": 12, "rpe": 9},
                    ],
                },
                {
                    "exercise_id": PULL_UP, "order_index": 0, "notes": null,
                    "sets": [
                        {"set_number": 1, "reps": 5, "weight_kg": 12.5, "rpe": 9},
                        {"set_number": 2, "reps": 4, "weight_kg": 12.5, "rpe": 10},
                    ],
                },
            ],
        }),
    )
    .await;
    let mobility = log_workout(
        &app,
        json!({"date": "2026-09-28", "name": "Mobility", "notes": null, "exercises": []}),
    )
    .await;
    // Logged in the evening of the 29th; the NOOP session after midnight stays on the 30th.
    let push = log_workout(
        &app,
        json!({
            "date": "2026-09-29", "name": "Push", "notes": null,
            "exercises": [{
                "exercise_id": RING_DIP, "order_index": 0, "notes": null,
                "sets": [{"set_number": 1, "reps": 8, "weight_kg": 20.0, "rpe": 8}],
            }],
        }),
    )
    .await;

    let body = context_ok(&app, "2026-09-28", "2026-10-01").await;
    let covered = sync("confirmed", "confirmed", "covered");
    // A day without NOOP values: the rows are covered, so the nulls are recorded absences.
    let without_recovery = |day: &str, workouts: Value, body_weight: Value| {
        json!({
            "day": day,
            "workouts": workouts,
            "noop_workouts": [],
            "body_weight": body_weight,
            "sleep_duration": measurement(None, "min", None),
            "resting_hr": measurement(None, "beats/min", None),
            "hrv_rmssd": measurement(None, "ms", None),
            "noop": covered,
        })
    };
    let expected_days = json!([
        {
            "day": "2026-09-28",
            "workouts": [
                {"id": pull, "name": "Pull", "exercises": [
                    {"exercise": "Pull-up", "sets": 2, "best_reps": 5,
                     "best_added_weight_kg": 12.5, "best_hold_duration_s": null, "best_rpe": 10,
                     "volume_kg": 112.5},
                    {"exercise": "Planche Hold", "sets": 2, "best_reps": null,
                     "best_added_weight_kg": null, "best_hold_duration_s": 15, "best_rpe": 9,
                     "volume_kg": 0.0},
                ]},
                {"id": mobility, "name": "Mobility", "exercises": []},
            ],
            "noop_workouts": [],
            "body_weight": measurement(Some(80.0), "kg", Some("body_weight")),
            "sleep_duration": measurement(Some(420.0), "min", Some(IMPORTED)),
            "resting_hr": measurement(Some(50.0), "beats/min", Some(IMPORTED)),
            "hrv_rmssd": measurement(Some(62.0), "ms", Some(COMPUTED)),
            "noop": covered,
        },
        without_recovery(
            "2026-09-29",
            json!([{"id": push, "name": "Push", "exercises": [
                {"exercise": "Ring Dip", "sets": 1, "best_reps": 8, "best_added_weight_kg": 20.0,
                 "best_hold_duration_s": null, "best_rpe": 8, "volume_kg": 160.0},
            ]}]),
            measurement(None, "kg", None),
        ),
        {
            "day": "2026-09-30",
            "workouts": [],
            "noop_workouts": [{
                "start": "2026-09-30T00:30:00+02:00", "end": "2026-09-30T01:30:00+02:00",
                "sport": "Calisthenics", "origin": "manual", "duration_min": 60.0,
                "avg_hr_bpm": 90.0, "strain_score": 12.0, "steps": null, "source": IMPORTED,
            }],
            "body_weight": measurement(None, "kg", None),
            "sleep_duration": measurement(Some(330.0), "min", Some(COMPUTED)),
            "resting_hr": measurement(None, "beats/min", None),
            "hrv_rmssd": measurement(None, "ms", None),
            "noop": covered,
        },
        without_recovery(
            "2026-10-01",
            json!([]),
            measurement(Some(79.5), "kg", Some("body_weight")),
        ),
    ]);
    assert_eq!(
        body,
        json!({
            "from_day": "2026-09-28",
            "to_day": "2026-10-01",
            "time_zone": "Europe/Copenhagen",
            "noop": {
                "installation_id": SOURCE,
                "imported_device_id": IMPORTED,
                "computed_device_id": COMPUTED,
                "last_push_at": {
                    "recovery": "2026-10-02T04:00:00+02:00",
                    "workouts": "2026-10-02T04:00:00+02:00",
                },
            },
            "days": expected_days,
        })
    );
}

#[tokio::test]
async fn keeps_logged_training_when_noop_has_never_pushed() {
    let app = common::test_app_with_noop_db(Mirror::empty().await.db).await;
    let workout = log_workout(
        &app,
        json!({"date": "2026-09-29", "name": "Legs", "notes": null, "exercises": []}),
    )
    .await;

    let body = context_ok(&app, "2026-09-28", "2026-09-29").await;
    assert_eq!(
        body["noop"],
        json!({
            "installation_id": null,
            "imported_device_id": IMPORTED,
            "computed_device_id": COMPUTED,
            "last_push_at": {"recovery": null, "workouts": null},
        })
    );
    let days = body["days"].as_array().unwrap();
    assert_eq!(days.len(), 2);
    assert_eq!(days[0]["workouts"], json!([]));
    assert_eq!(
        days[1]["workouts"],
        json!([{"id": workout, "name": "Legs", "exercises": []}])
    );
    for entry in days {
        assert_eq!(entry["noop_workouts"], json!([]));
        assert_eq!(entry["sleep_duration"]["value"], Value::Null);
        assert_eq!(entry["hrv_rmssd"]["source"], Value::Null);
        assert_eq!(entry["noop"], sync("unknown", "unknown", "unknown"));
    }
}

#[tokio::test]
async fn reports_recovery_and_workout_freshness_separately() {
    let mut mirror = Mirror::empty().await;
    let from_ts = day("2026-09-28").start().timestamp();
    let to_ts = day("2026-09-30").start().timestamp();
    // Only the workout stream has arrived, during the second day.
    for device in [IMPORTED, COMPUTED] {
        mirror
            .push(
                SOURCE,
                device,
                Window::WorkoutStarts(from_ts, to_ts),
                &[],
                "2026-09-29T10:00:00.000Z",
            )
            .await;
    }
    let app = common::test_app_with_noop_db(mirror.db).await;

    let body = context_ok(&app, "2026-09-28", "2026-09-29").await;
    assert_eq!(body["noop"]["installation_id"], SOURCE);
    assert_eq!(
        body["noop"]["last_push_at"],
        json!({"recovery": null, "workouts": "2026-09-29T12:00:00+02:00"})
    );
    let recovery = json!({
        "freshness": "unknown",
        "coverage": {"daily_metrics": "unknown", "sleep_sessions": "unknown"},
    });
    for (entry, workouts) in [(0, "confirmed"), (1, "partial")] {
        assert_eq!(
            body["days"][entry]["noop"],
            json!({
                "recovery": recovery,
                "workouts": {"freshness": workouts, "coverage": {"workouts": "covered"}},
            })
        );
    }
}

#[tokio::test]
async fn edited_sleep_selects_the_computed_night() {
    let mut mirror = mirror(
        "2026-09-14",
        "2026-09-17",
        &[daily(
            "2026-09-15",
            json!({"totalSleepMin": 400.0, "efficiency": 0.8}),
        )],
        &[daily(
            "2026-09-15",
            json!({"totalSleepMin": 102.25, "efficiency": 0.22}),
        )],
        &[],
        "2026-09-17T02:00:00.000Z",
    )
    .await;
    let start = at("2026-09-15", Duration::minutes(83));
    let session = json!({
        "type": "record", "key": {"startTs": start},
        "data": {"endTs": start + 7 * 3600, "efficiency": null, "restingHr": null,
            "avgHrv": null, "stagesJSON": null, "userEdited": true, "startTsAdjusted": null,
            "motionJSON": null, "sleepStateJSON": null, "stagingSparse": null},
    });
    let starts = Window::Starts(
        day("2026-09-14").start().timestamp(),
        day("2026-09-17").start().timestamp(),
    );
    mirror
        .push(
            SOURCE,
            COMPUTED,
            starts,
            &[session],
            "2026-09-17T02:00:00.000Z",
        )
        .await;
    let app = common::test_app_with_noop_db(mirror.db).await;

    let body = context_ok(&app, "2026-09-15", "2026-09-15").await;
    assert_eq!(
        body["days"][0]["sleep_duration"],
        measurement(Some(102.25), "min", Some(COMPUTED))
    );
}
