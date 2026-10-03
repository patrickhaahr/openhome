//! Workouts on a Training Day (`GET /api/training/days/{day}/workouts`): Workouts logged through
//! the fitness API beside NOOP Workouts pushed through the real push endpoint.

mod common;

use axum::Router;
use axum::http::{Method, StatusCode};
use common::mirror::{Mirror, Window, noop_workout};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

const API_KEY: &str = "test-api-key";

/// The production installation and NOOP's strap namespaces.
const SOURCE: &str = "81906e30-187d-4546-8f8a-9949b82d62fa";
const IMPORTED: &str = "my-whoop";
const COMPUTED: &str = "my-whoop-noop";

/// The production mirror's latest push: 03:51 local time on 2026-09-26.
const REAL_PUSH_AT: &str = "2026-09-26T01:51:38.318Z";
/// NOOP's real 14-day workout window ending with 2026-09-26 (local midnights).
const REAL_STARTS: Window = Window::WorkoutStarts(1_789_250_400, 1_790_460_000);

/// Seeded exercise library ids.
const PULL_UP: i64 = 11;
const RING_DIP: i64 = 10;
const PLANCHE_HOLD: i64 = 1;

/// The real NOOP Workout: Calisthenics timed manually in NOOP, 02:12:38 → 03:30:11 on 2026-09-26.
fn real_noop_workout() -> Value {
    noop_workout(
        1_790_381_558,
        1_790_386_211,
        "Calisthenics",
        "manual",
        json!({
            "durationS": 4653.247, "energyKcal": 400.8306717632, "avgHr": 81.0, "maxHr": 134.0,
            "strain": 24.51,
        }),
    )
}

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

/// Both namespaces pushed `workout` windows with the given rows, accepted at `accepted_at`.
async fn mirror_with(imported: &[Value], computed: &[Value], accepted_at: &str) -> Mirror {
    let mut mirror = Mirror::empty().await;
    mirror
        .push(SOURCE, IMPORTED, REAL_STARTS, imported, accepted_at)
        .await;
    mirror
        .push(SOURCE, COMPUTED, REAL_STARTS, computed, accepted_at)
        .await;
    mirror
}

/// The API over `mirror` with an empty training log.
async fn app(mirror: &Mirror) -> Router {
    common::test_app_with_noop_db(mirror.db.clone()).await
}

/// Logs a Workout through the fitness API and returns its id.
async fn log_workout(app: &Router, workout: Value) -> i64 {
    let (status, body) = common::send_request_with_method(
        app.clone(),
        "/api/workouts",
        Method::POST,
        Some(workout),
        Some(API_KEY),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body["id"].as_i64().unwrap()
}

async fn workouts(app: &Router, day: &str, api_key: Option<&str>) -> (StatusCode, Value) {
    common::send_request(
        app.clone(),
        &format!("/api/training/days/{day}/workouts"),
        api_key,
    )
    .await
}

async fn workouts_on(app: &Router, day: &str) -> Value {
    let (status, body) = workouts(app, day, Some(API_KEY)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body
}

/// `expected` after the JSON text round trip the response body takes: `serde_json`'s default float
/// parser may land one ulp away from the exact quotient the test computes.
fn as_parsed(expected: &Value) -> Value {
    serde_json::from_str(&expected.to_string()).unwrap()
}

fn starts(body: &Value) -> Vec<&str> {
    body["noop_workouts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|workout| workout["start"].as_str().unwrap())
        .collect()
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

#[tokio::test]
async fn a_late_noop_sync_adds_workouts_beside_the_logged_ones() {
    let morning_run = noop_workout(1_790_398_800, 1_790_401_500, "Running", "manual", json!({}));
    // 08:00 local on the 26th: the phone pushed the morning run during the day.
    let mut mirror = mirror_with(
        std::slice::from_ref(&morning_run),
        &[],
        "2026-09-26T06:00:00.000Z",
    )
    .await;
    let app = app(&mirror).await;
    let logged = log_workout(
        &app,
        json!({
            "date": "2026-09-26", "name": "Pull", "notes": null,
            "exercises": [{"exercise_id": PULL_UP, "order_index": 0, "notes": null,
                           "sets": [{"set_number": 1, "reps": 6}]}],
        }),
    )
    .await;

    let before = workouts_on(&app, "2026-09-26").await;
    assert_eq!(starts(&before), ["2026-09-26T07:00:00+02:00"]);
    assert_eq!(before["noop"]["freshness"], "partial");

    // The next day's push replaces only the changed day and brings the evening session.
    let changed_day = Window::WorkoutStarts(1_790_373_600, 1_790_460_000);
    let evening = noop_workout(
        1_790_438_400,
        1_790_442_600,
        "Calisthenics",
        "manual",
        json!({}),
    );
    mirror
        .push(
            SOURCE,
            IMPORTED,
            changed_day,
            &[morning_run, evening],
            "2026-09-27T03:00:00.000Z",
        )
        .await;
    mirror
        .push(
            SOURCE,
            COMPUTED,
            changed_day,
            &[],
            "2026-09-27T03:00:00.000Z",
        )
        .await;

    let after = workouts_on(&app, "2026-09-26").await;
    assert_eq!(
        starts(&after),
        ["2026-09-26T07:00:00+02:00", "2026-09-26T18:00:00+02:00"]
    );
    assert_eq!(after["workouts"], before["workouts"]);
    assert_eq!(after["workouts"][0]["id"], logged);
    assert_eq!(after["noop"]["freshness"], "confirmed");
    assert_eq!(after["noop"]["last_push_at"], "2026-09-27T05:00:00+02:00");
}

#[tokio::test]
async fn missing_noop_workouts_are_covered_or_unknown() {
    let logged = json!({
        "date": "2026-09-20", "name": null, "notes": null,
        "exercises": [{"exercise_id": PULL_UP, "order_index": 0, "notes": null,
                       "sets": [{"set_number": 1, "reps": 6}]}],
    });

    // Before any push nothing is known about NOOP.
    let unsynced = app(&Mirror::empty().await).await;
    log_workout(&unsynced, logged.clone()).await;
    let body = workouts_on(&unsynced, "2026-09-20").await;
    assert_eq!(body["workouts"].as_array().unwrap().len(), 1);
    assert_eq!(body["noop_workouts"], json!([]));
    assert_eq!(
        body["noop"],
        json!({
            "installation_id": null,
            "imported_device_id": IMPORTED,
            "computed_device_id": COMPUTED,
            "last_push_at": null,
            "freshness": "unknown",
            "coverage": {"workouts": "unknown"},
        })
    );

    // Both namespaces pushed the day without a workout: NOOP recorded none.
    let covered = app(&mirror_with(&[], &[], REAL_PUSH_AT).await).await;
    log_workout(&covered, logged.clone()).await;
    let body = workouts_on(&covered, "2026-09-20").await;
    assert_eq!(body["workouts"].as_array().unwrap().len(), 1);
    assert_eq!(body["noop_workouts"], json!([]));
    assert_eq!(body["noop"]["coverage"], json!({"workouts": "covered"}));
    assert_eq!(body["noop"]["freshness"], "confirmed");

    // Only one namespace pushed workouts, so the day is not established.
    let mut half = Mirror::empty().await;
    half.push(SOURCE, IMPORTED, REAL_STARTS, &[], REAL_PUSH_AT)
        .await;
    let body = workouts_on(&app(&half).await, "2026-09-20").await;
    assert_eq!(body["noop"]["coverage"], json!({"workouts": "unknown"}));
    assert_eq!(body["noop"]["last_push_at"], Value::Null);
    assert_eq!(body["noop"]["freshness"], "unknown");

    // A day after the latest workout push is awaiting the phone's next sync.
    let body = workouts_on(&covered, "2026-09-27").await;
    assert_eq!(body["noop"]["coverage"], json!({"workouts": "unknown"}));
    assert_eq!(body["noop"]["freshness"], "unconfirmed");
}

#[tokio::test]
async fn workouts_are_grouped_by_copenhagen_start_day() {
    let mirror = mirror_with(
        &[
            // 23:30 on the 19th to 00:45 on the 20th: a Workout of the 19th.
            noop_workout(1_789_853_400, 1_789_857_900, "Running", "manual", json!({})),
            // 00:30 local on the 20th is still the 19th in UTC.
            noop_workout(1_789_857_000, 1_789_860_600, "Cycling", "manual", json!({})),
        ],
        &[],
        REAL_PUSH_AT,
    )
    .await;
    let app = app(&mirror).await;

    let sep_19 = workouts_on(&app, "2026-09-19").await;
    assert_eq!(starts(&sep_19), ["2026-09-19T23:30:00+02:00"]);
    assert_eq!(
        sep_19["noop_workouts"][0]["end"],
        "2026-09-20T00:45:00+02:00"
    );
    let sep_20 = workouts_on(&app, "2026-09-20").await;
    assert_eq!(starts(&sep_20), ["2026-09-20T00:30:00+02:00"]);
}

#[tokio::test]
async fn day_bounds_and_offsets_follow_daylight_saving() {
    // Summer time ends at 03:00 CEST on 2026-10-25; that local day lasts 25 hours.
    let october = Window::WorkoutStarts(1_791_756_000, 1_792_969_200);
    let mut mirror = Mirror::empty().await;
    let rows = [
        // 02:30 after the clocks went back, and 23:30 CET: both still on the 25th.
        noop_workout(1_792_891_800, 1_792_895_400, "Running", "manual", json!({})),
        noop_workout(1_792_967_400, 1_792_971_000, "Walking", "manual", json!({})),
    ];
    mirror
        .push(SOURCE, IMPORTED, october, &rows, "2026-10-26T06:00:00.000Z")
        .await;
    mirror
        .push(SOURCE, COMPUTED, october, &[], "2026-10-26T06:00:00.000Z")
        .await;

    let body = workouts_on(&app(&mirror).await, "2026-10-25").await;
    assert_eq!(body["day_start"], "2026-10-25T00:00:00+02:00");
    assert_eq!(body["day_end"], "2026-10-26T00:00:00+01:00");
    assert_eq!(
        starts(&body),
        ["2026-10-25T02:30:00+01:00", "2026-10-25T23:30:00+01:00"]
    );
    assert_eq!(body["noop"]["coverage"], json!({"workouts": "covered"}));
    assert_eq!(body["noop"]["freshness"], "confirmed");
}

#[tokio::test]
async fn includes_imported_workouts_and_requires_their_coverage() {
    let mut mirror = mirror_with(&[], &[], REAL_PUSH_AT).await;
    let sources = ["apple-health", "health-connect", "lifting", "activity-file"];
    for (index, device) in sources.iter().enumerate() {
        let start = 1_790_400_000 + index as i64 * 3_600;
        mirror
            .push(
                SOURCE,
                device,
                Window::WorkoutStarts(start, start + 600),
                &[noop_workout(
                    start,
                    start + 600,
                    "Running",
                    device,
                    json!({}),
                )],
                "2026-09-25T20:00:00.000Z",
            )
            .await;
    }
    let app = app(&mirror).await;
    let before = workouts_on(&app, "2026-09-26").await;
    let returned_sources: Vec<&str> = before["noop_workouts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["source"].as_str().unwrap())
        .collect();
    assert_eq!(returned_sources, sources);
    assert_eq!(before["noop"]["coverage"]["workouts"], "unknown");
    assert_eq!(before["noop"]["freshness"], "unconfirmed");
    assert_eq!(before["noop"]["last_push_at"], "2026-09-25T22:00:00+02:00");

    // Complete empty windows remove the imported sessions and establish recorded absence.
    for device in sources {
        mirror
            .push(SOURCE, device, REAL_STARTS, &[], REAL_PUSH_AT)
            .await;
    }
    let after = workouts_on(&app, "2026-09-26").await;
    assert_eq!(after["noop_workouts"], json!([]));
    assert_eq!(after["noop"]["coverage"]["workouts"], "covered");
    assert_eq!(after["noop"]["freshness"], "partial");
}

#[tokio::test]
async fn staged_imports_keep_workout_coverage_unknown() {
    let mut mirror = mirror_with(&[], &[], REAL_PUSH_AT).await;
    mirror
        .push_first_part(
            SOURCE,
            "activity-file",
            REAL_STARTS,
            &[real_noop_workout()],
            2,
            REAL_PUSH_AT,
        )
        .await;

    let body = workouts_on(&app(&mirror).await, "2026-09-26").await;
    assert_eq!(body["noop_workouts"], json!([]));
    assert_eq!(body["noop"]["coverage"]["workouts"], "unknown");
    assert_eq!(body["noop"]["last_push_at"], Value::Null);
    assert_eq!(body["noop"]["freshness"], "unknown");
}

#[tokio::test]
async fn invalid_noop_timestamps_return_an_error_instead_of_panicking() {
    let mirror = mirror_with(
        &[
            noop_workout(1_790_381_558, i64::MIN, "Calisthenics", "manual", json!({})),
            noop_workout(
                1_790_438_400,
                1_790_442_600,
                "Calisthenics",
                "manual",
                json!({}),
            ),
        ],
        &[],
        REAL_PUSH_AT,
    )
    .await;

    let (status, body) = workouts(&app(&mirror).await, "2026-09-26", Some(API_KEY)).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(body["status"], 500);
}

#[tokio::test]
async fn rejects_days_over_the_caps_instead_of_truncating() {
    let rows: Vec<Value> = (0..25)
        .map(|n| {
            let start = 1_790_373_600 + n * 3_000;
            noop_workout(start, start + 600, "Running", "manual", json!({}))
        })
        .collect();
    let mirror = mirror_with(&rows, &[], REAL_PUSH_AT).await;
    let (status, body) = workouts(&app(&mirror).await, "2026-09-26", Some(API_KEY)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        body,
        json!({
            "error": "More than 24 NOOP workouts start on 2026-09-26; refusing to return a \
                      truncated day",
            "status": 422,
        })
    );

    let app = app(&Mirror::empty().await).await;
    let sets: Vec<Value> = (1..=501)
        .map(|n| json!({"set_number": n, "reps": 1}))
        .collect();
    log_workout(
        &app,
        json!({
            "date": "2026-09-26", "name": null, "notes": null,
            "exercises": [{"exercise_id": PULL_UP, "order_index": 0, "notes": null,
                           "sets": sets}],
        }),
    )
    .await;
    let (status, body) = workouts(&app, "2026-09-26", Some(API_KEY)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        body["error"],
        "The Workouts logged on 2026-09-26 have more than 500 Set rows; refusing to return a \
         truncated day"
    );
}
