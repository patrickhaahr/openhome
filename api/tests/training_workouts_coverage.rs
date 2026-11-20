mod common;
mod training_workouts_support;

use axum::http::StatusCode;
use common::mirror::Mirror;
use common::mirror::Window;
use common::mirror::noop_workout;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use training_workouts_support::{
    API_KEY, COMPUTED, IMPORTED, PULL_UP, REAL_PUSH_AT, REAL_STARTS, SOURCE, app, log_workout,
    mirror_with, real_noop_workout, workouts, workouts_on,
};

fn starts(body: &Value) -> Vec<&str> {
    body["noop_workouts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|workout| workout["start"].as_str().unwrap())
        .collect()
}

#[tokio::test]
async fn a_late_noop_sync_adds_workouts_beside_the_logged_ones() {
    let morning_run = noop_workout(1_790_398_800, 1_790_401_500, "Running", "manual", json!({}));
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

    let covered = app(&mirror_with(&[], &[], REAL_PUSH_AT).await).await;
    log_workout(&covered, logged.clone()).await;
    let body = workouts_on(&covered, "2026-09-20").await;
    assert_eq!(body["workouts"].as_array().unwrap().len(), 1);
    assert_eq!(body["noop_workouts"], json!([]));
    assert_eq!(body["noop"]["coverage"], json!({"workouts": "covered"}));
    assert_eq!(body["noop"]["freshness"], "confirmed");

    let mut half = Mirror::empty().await;
    half.push(SOURCE, IMPORTED, REAL_STARTS, &[], REAL_PUSH_AT)
        .await;
    let body = workouts_on(&app(&half).await, "2026-09-20").await;
    assert_eq!(body["noop"]["coverage"], json!({"workouts": "unknown"}));
    assert_eq!(body["noop"]["last_push_at"], Value::Null);
    assert_eq!(body["noop"]["freshness"], "unknown");

    let body = workouts_on(&covered, "2026-09-27").await;
    assert_eq!(body["noop"]["coverage"], json!({"workouts": "unknown"}));
    assert_eq!(body["noop"]["freshness"], "unconfirmed");
}

#[tokio::test]
async fn workouts_are_grouped_by_copenhagen_start_day() {
    let mirror = mirror_with(
        &[
            noop_workout(1_789_853_400, 1_789_857_900, "Running", "manual", json!({})),
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
    let october = Window::WorkoutStarts(1_791_756_000, 1_792_969_200);
    let mut mirror = Mirror::empty().await;
    let rows = [
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
