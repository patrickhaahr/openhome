//! NOOP Workout Detail (`GET /api/training/noop-workouts/{source}/{start}`): one NOOP Workout,
//! selected by its listed `source` and `start`, with heart rate from the strap's samples. Workouts
//! and samples are pushed through the real push endpoint.

mod common;

use axum::Router;
use axum::http::{Method, StatusCode};
use common::mirror::{Mirror, noop_workout};
use common::reference_run::{self, COMPUTED, IMPORTED, PUSHED_AT, SOURCE, WORKOUT_WINDOW};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

const API_KEY: &str = "test-api-key";

/// The API over `mirror` with an empty training log.
async fn app(mirror: &Mirror) -> Router {
    common::test_app_with_noop_db(mirror.db.clone()).await
}

async fn detail(
    app: &Router,
    source: &str,
    start: &str,
    api_key: Option<&str>,
) -> (StatusCode, Value) {
    common::send_request(
        app.clone(),
        &format!("/api/training/noop-workouts/{source}/{start}"),
        api_key,
    )
    .await
}

async fn detail_of(app: &Router, source: &str, start: &str) -> Value {
    let (status, body) = detail(app, source, start, Some(API_KEY)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body
}

/// Sets the Profile's Max Heart Rate, or clears it with `None`.
async fn set_hr_max(app: &Router, hr_max_bpm: Option<i64>) {
    let (status, body) = common::send_request_with_method(
        app.clone(),
        "/api/profile",
        Method::PATCH,
        Some(json!({"hr_max_bpm": hr_max_bpm})),
        Some(API_KEY),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
}

/// Workouts on the Training Day `day`.
async fn workouts_on(app: &Router, day: &str) -> Value {
    let (status, body) = common::send_request(
        app.clone(),
        &format!("/api/training/days/{day}/workouts"),
        Some(API_KEY),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body
}

/// Asserts `actual` is a number within 1e-9 of `expected`.
fn assert_close(actual: &Value, expected: f64) {
    let actual = actual
        .as_f64()
        .unwrap_or_else(|| panic!("{actual} is not a number"));
    assert!((actual - expected).abs() < 1e-9, "{actual} != {expected}");
}

#[tokio::test]
async fn the_reference_run_reports_heart_rate_from_the_strap_samples() {
    let app = app(&reference_run::mirror().await).await;

    let mut body = detail_of(&app, IMPORTED, reference_run::START).await;

    // The strap's 2,740 samples in [start, end): the app's 166 / 185, not the row's 160 / 183.
    assert_eq!(body["hr"]["basis"], "strap_samples");
    assert_close(&body["hr"]["avg_bpm"], 166.075912408759);
    assert_eq!(body["hr"]["min_bpm"], 84.0);
    assert_eq!(body["hr"]["max_bpm"], 185.0);
    // 2739.582 s over 7.169 km: 6:22 per km.
    assert_close(&body["avg_pace_s_per_km"], 382.12053039235946);
    body["hr"]["avg_bpm"] = Value::Null;
    body["avg_pace_s_per_km"] = Value::Null;
    // Bucket behavior is covered by the short/gapped and long-session HTTP cases.
    body["hr_series"] = Value::Null;
    assert_eq!(
        body,
        json!({
            "start": reference_run::START,
            "end": reference_run::END,
            "sport": "Running",
            "origin": "manual",
            "source": IMPORTED,
            "duration_min": 45.6597,
            "distance_m": 7169.41850045851,
            "energy_kcal": 817.170682229156,
            "strain_score": 58.45,
            "avg_pace_s_per_km": null,
            "hr": {"basis": "strap_samples", "avg_bpm": null, "min_bpm": 84.0, "max_bpm": 185.0},
            "hr_series": null,
            // Without a Profile Max Heart Rate the zones and recovery are never guessed.
            "hr_zones": null,
            "hr_recovery": null,
            "unavailable": {"hr_zones": "hr_max_not_configured", "hr_recovery": "hr_max_not_configured"},
            "noop": {
                "installation_id": SOURCE,
                "imported_device_id": IMPORTED,
                "computed_device_id": COMPUTED,
                "last_push_at": "2026-09-28T06:00:00+02:00",
                "freshness": "confirmed",
                "coverage": {"workouts": "covered"},
            },
        })
    );
}

#[tokio::test]
async fn the_reference_run_splits_into_the_apps_heart_rate_zones() {
    let app = app(&reference_run::mirror().await).await;
    set_hr_max(&app, Some(193)).await;

    let body = detail_of(&app, IMPORTED, reference_run::START).await;

    let zones = &body["hr_zones"];
    assert_eq!(zones["hrmax_used"], 193);
    let bands: Vec<(i64, i64, i64)> = zones["zones"]
        .as_array()
        .unwrap()
        .iter()
        .map(|zone| {
            (
                zone["zone"].as_i64().unwrap(),
                zone["lower_bpm"].as_i64().unwrap(),
                zone["upper_bpm"].as_i64().unwrap(),
            )
        })
        .collect();
    // 50/60/70/80/90 % of 193 is 96.5/115.8/135.1/154.4/173.7 bpm.
    assert_eq!(
        bands,
        [
            (1, 97, 115),
            (2, 116, 135),
            (3, 136, 154),
            (4, 155, 173),
            (5, 174, 193)
        ]
    );
    // The NOOP app shows 1 / 4 / 6 / 39 / 50 % and 0 / 1 / 2 / 17 / 21 whole minutes.
    let rounded_percent: Vec<i64> = zones["zones"]
        .as_array()
        .unwrap()
        .iter()
        .map(|zone| zone["percent"].as_f64().unwrap().round() as i64)
        .collect();
    assert_eq!(rounded_percent, [1, 4, 6, 39, 50]);
    let floored_minutes: Vec<i64> = zones["zones"]
        .as_array()
        .unwrap()
        .iter()
        .map(|zone| zone["minutes"].as_f64().unwrap().floor() as i64)
        .collect();
    assert_eq!(floored_minutes, [0, 1, 2, 17, 21]);
    assert_eq!(body["unavailable"], json!({}));
}

#[tokio::test]
async fn the_reference_run_reports_the_apps_heart_rate_recovery() {
    let app = app(&reference_run::mirror().await).await;
    set_hr_max(&app, Some(193)).await;

    let body = detail_of(&app, IMPORTED, reference_run::START).await;

    // The NOOP app shows 183 bpm at the end, then drops of 31, 44 and 49 bpm.
    assert_eq!(
        body["hr_recovery"],
        json!({
            "end_hr_bpm": 183,
            "at_1_min": {"hr_bpm": 152, "drop_bpm": 31},
            "at_2_min": {"hr_bpm": 139, "drop_bpm": 44},
            "at_5_min": {"hr_bpm": 134, "drop_bpm": 49},
        })
    );
    assert_eq!(body["unavailable"], json!({}));

    // Clearing Max Heart Rate takes recovery away at once.
    set_hr_max(&app, None).await;
    let body = detail_of(&app, IMPORTED, reference_run::START).await;
    assert_eq!(body.get("hr_recovery"), Some(&Value::Null));
    assert_eq!(
        body["unavailable"],
        json!({"hr_zones": "hr_max_not_configured", "hr_recovery": "hr_max_not_configured"})
    );
}

#[tokio::test]
async fn repeats_the_listed_workout_and_its_training_day_sync() {
    let app = app(&reference_run::mirror().await).await;
    let day = workouts_on(&app, "2026-09-27").await;
    let listed = &day["noop_workouts"][0];

    let body = detail_of(
        &app,
        listed["source"].as_str().unwrap(),
        listed["start"].as_str().unwrap(),
    )
    .await;
    for field in [
        "start",
        "end",
        "sport",
        "origin",
        "source",
        "duration_min",
        "distance_m",
        "energy_kcal",
        "strain_score",
    ] {
        assert_eq!(body[field], listed[field], "{field}");
    }
    assert_eq!(body["noop"], day["noop"]);
    // The route stays in the database.
    assert!(!body.to_string().contains("route-stand-in"), "{body}");
}

#[tokio::test]
async fn start_is_matched_as_an_instant_whatever_its_offset() {
    let app = app(&reference_run::mirror().await).await;
    let listed = detail_of(&app, IMPORTED, reference_run::START).await;

    for start in [
        "2026-09-27T14:33:52Z",
        "2026-09-27T14:33:52.000Z",
        "2026-09-27T09:33:52-05:00",
    ] {
        assert_eq!(detail_of(&app, IMPORTED, start).await, listed, "{start}");
    }
}

#[tokio::test]
async fn opens_only_workouts_in_the_merged_list() {
    let manual = noop_workout(
        1_790_438_400,
        1_790_442_600,
        "Calisthenics",
        "manual",
        json!({"avgHr": 96.0}),
    );
    let mut mirror = Mirror::empty().await;
    mirror
        .push(
            SOURCE,
            IMPORTED,
            WORKOUT_WINDOW,
            std::slice::from_ref(&manual),
            PUSHED_AT,
        )
        .await;
    mirror
        .push(
            SOURCE,
            COMPUTED,
            WORKOUT_WINDOW,
            &[
                // The same row in the computed namespace, and the detector's wider shadow of it.
                manual.clone(),
                noop_workout(
                    1_790_437_800,
                    1_790_443_200,
                    "detected",
                    COMPUTED,
                    json!({"avgHr": 90.0}),
                ),
            ],
            PUSHED_AT,
        )
        .await;
    let app = app(&mirror).await;
    let listed = workouts_on(&app, "2026-09-26").await["noop_workouts"].clone();
    assert_eq!(listed.as_array().unwrap().len(), 1);
    assert_eq!(listed[0]["source"], IMPORTED);
    assert_eq!(listed[0]["start"], "2026-09-26T18:00:00+02:00");
    detail_of(&app, IMPORTED, "2026-09-26T18:00:00+02:00").await;

    for (source, start) in [
        // NOOP folds the computed copy into the imported row.
        (COMPUTED, "2026-09-26T18:00:00+02:00"),
        // NOOP drops the detected bout as a shadow of the manual session.
        (COMPUTED, "2026-09-26T17:50:00+02:00"),
        // Nothing starts a second later, or in another namespace.
        (IMPORTED, "2026-09-26T18:00:01+02:00"),
        ("apple-health", "2026-09-26T18:00:00+02:00"),
    ] {
        let (status, body) = detail(&app, source, start, Some(API_KEY)).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{source} {start}: {body}");
        assert_eq!(
            body,
            json!({
                "error": format!("No NOOP Workout from '{source}' starts at {start}"),
                "status": 404,
            })
        );
    }

    // Before any strap push there is no installation to open a workout from.
    let (status, _) = detail(
        &common::test_app_with_noop_db(Mirror::empty().await.db).await,
        IMPORTED,
        "2026-09-26T18:00:00+02:00",
        Some(API_KEY),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn requires_the_api_key() {
    let app = app(&reference_run::mirror().await).await;
    for api_key in [None, Some("wrong-key")] {
        let (status, body) = detail(&app, IMPORTED, reference_run::START, api_key).await;

        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(
            body,
            json!({"error": "Missing or invalid API key", "status": 401})
        );
    }
}

#[tokio::test]
async fn rejects_a_start_that_is_not_rfc_3339() {
    let app = app(&reference_run::mirror().await).await;
    for start in [
        "2026-09-27",
        "2026-09-27T16:33:52",
        "2026-09-27T25:33:52+02:00",
        "1790519632",
        "2026-02-30T16:33:52+02:00",
    ] {
        let (status, body) = detail(&app, IMPORTED, start, Some(API_KEY)).await;

        assert_eq!(status, StatusCode::BAD_REQUEST, "{start}");
        assert_eq!(
            body,
            json!({
                "error": format!(
                    "Invalid start '{start}': must be an RFC 3339 timestamp with an offset, as \
                     workouts_on_day lists it"
                ),
                "status": 400,
            })
        );
    }
}

#[tokio::test]
async fn rejects_an_empty_or_malformed_source() {
    let app = app(&reference_run::mirror().await).await;
    for (source, shown) in [("", ""), ("my-whoop%0A", "my-whoop\n"), ("%09", "\t")] {
        let (status, body) = detail(&app, source, reference_run::START, Some(API_KEY)).await;

        assert_eq!(status, StatusCode::BAD_REQUEST, "{source}: {body}");
        assert_eq!(
            body,
            json!({
                "error": format!(
                    "Invalid source {shown:?}: must be a NOOP device namespace as \
                     workouts_on_day lists it"
                ),
                "status": 400,
            })
        );
    }
}

/// A mirror where only the strap's imported namespace pushed `rows` in the workout window.
async fn mirror_with(rows: &[Value]) -> Mirror {
    let mut mirror = Mirror::empty().await;
    mirror
        .push(SOURCE, IMPORTED, WORKOUT_WINDOW, rows, PUSHED_AT)
        .await;
    mirror
}

#[tokio::test]
async fn rejects_oversized_workouts_and_days_instead_of_truncating() {
    // 08:00 on the 26th until 08:00:01 on the 27th: one second over a day.
    let long = noop_workout(1_790_402_400, 1_790_488_801, "Hiking", "manual", json!({}));
    let exactly_a_day = noop_workout(1_790_406_000, 1_790_492_400, "Walking", "manual", json!({}));
    let long_app = app(&mirror_with(&[long, exactly_a_day]).await).await;
    detail_of(&long_app, IMPORTED, "2026-09-26T09:00:00+02:00").await;
    let (status, body) = detail(
        &long_app,
        IMPORTED,
        "2026-09-26T08:00:00+02:00",
        Some(API_KEY),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        body,
        json!({
            "error": "The NOOP Workout from 'my-whoop' starting at 2026-09-26T08:00:00+02:00 \
                      lasts more than 24 hours; refusing to read its heart rate",
            "status": 422,
        })
    );

    let crowded: Vec<Value> = (0..25)
        .map(|n| {
            let start = 1_790_373_600 + n * 3_000;
            noop_workout(start, start + 600, "Running", "manual", json!({}))
        })
        .collect();
    let crowded_app = app(&mirror_with(&crowded).await).await;
    let (status, body) = detail(
        &crowded_app,
        IMPORTED,
        "2026-09-26T00:00:00+02:00",
        Some(API_KEY),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        body,
        json!({
            "error": "More than 24 NOOP workouts start on 2026-09-26; refusing to return a \
                      truncated day",
            "status": 422,
        })
    );
}

#[tokio::test]
async fn heart_rate_comes_from_the_strap_whatever_namespace_recorded_the_workout() {
    // 07:00:01-07:01:01 on the 26th, off Unix bucket boundaries, imported from Apple Health.
    let mut mirror = mirror_with(&[]).await;
    mirror
        .push(
            SOURCE,
            "apple-health",
            WORKOUT_WINDOW,
            &[noop_workout(
                1_790_398_801,
                1_790_398_861,
                "Running",
                "apple-health",
                json!({"avgHr": 150.0, "maxHr": 170.0}),
            )],
            PUSHED_AT,
        )
        .await;
    let strap = [
        // Just before the start, inside [start, end), then at the end.
        (1_790_398_800, 200),
        (1_790_398_801, 120),
        (1_790_398_815, 123),
        (1_790_398_831, 141),
        (1_790_398_860, 130),
        (1_790_398_861, 40),
    ];
    mirror
        .push_hr_samples(SOURCE, IMPORTED, &strap, PUSHED_AT)
        .await;
    // Neither the computed namespace, another namespace nor another installation is the strap.
    mirror
        .push_hr_samples(SOURCE, COMPUTED, &[(1_790_398_810, 30)], PUSHED_AT)
        .await;
    mirror
        .push_hr_samples(SOURCE, "apple-health", &[(1_790_398_811, 31)], PUSHED_AT)
        .await;
    mirror
        .push_hr_samples(
            "6b1f0a52-0d6e-4a0b-9d3e-3f1c2b7a9e10",
            IMPORTED,
            &[(1_790_398_812, 32)],
            "2026-09-01T00:00:00.000Z",
        )
        .await;

    let body = detail_of(
        &app(&mirror).await,
        "apple-health",
        "2026-09-26T07:00:01+02:00",
    )
    .await;
    assert_eq!(body["origin"], "health_import");
    assert_eq!(body["source"], "apple-health");
    // Buckets start at the workout, not at Unix-aligned multiples of 15. The empty
    // [15, 30) bucket is omitted; boundary and post-workout samples never leak in.
    assert_eq!(
        body["hr_series"],
        json!({"bucket_s": 15, "points": [
            {"t_s": 0, "bpm": 121.5},
            {"t_s": 30, "bpm": 141.0},
            {"t_s": 45, "bpm": 130.0},
        ]})
    );
    assert_eq!(
        body["hr"],
        json!({"basis": "strap_samples", "avg_bpm": 128.5, "min_bpm": 120.0,
               "max_bpm": 141.0})
    );
}

#[tokio::test]
async fn heart_rate_series_widens_only_as_needed_to_keep_at_most_300_buckets() {
    // Exact thresholds, one second beyond them, and the longest accepted workout.
    for (duration_s, bucket_s, count, last_t_s) in [
        (4_500, 15, 300, 4_485),
        (4_501, 30, 151, 4_500),
        (9_000, 30, 300, 8_970),
        (9_001, 45, 201, 9_000),
        (86_400, 300, 288, 86_100),
    ] {
        let start = 1_790_398_800;
        let mut mirror = mirror_with(&[noop_workout(
            start,
            start + duration_s,
            "Running",
            "manual",
            // The recorded duration must not determine the series' time window.
            json!({"durationS": 60.0}),
        )])
        .await;
        let samples: Vec<_> = (0..duration_s)
            .step_by(15)
            .map(|offset| (start + offset, 120))
            .collect();
        mirror
            .push_hr_samples(SOURCE, IMPORTED, &samples, PUSHED_AT)
            .await;

        let body = detail_of(&app(&mirror).await, IMPORTED, "2026-09-26T07:00:00+02:00").await;
        let series = &body["hr_series"];
        assert_eq!(series["bucket_s"], bucket_s, "duration {duration_s}");
        let points = series["points"].as_array().unwrap();
        assert_eq!(points.len(), count, "duration {duration_s}");
        assert!(points.len() <= 300);
        assert_eq!(points.first().unwrap(), &json!({"t_s": 0, "bpm": 120.0}));
        assert_eq!(
            points.last().unwrap(),
            &json!({"t_s": last_t_s, "bpm": 120.0})
        );
        assert!(points.windows(2).all(|pair| {
            pair[1]["t_s"].as_i64().unwrap() - pair[0]["t_s"].as_i64().unwrap() == bucket_s
        }));
    }
}

#[tokio::test]
async fn heart_rate_falls_back_to_the_row_only_without_strap_samples() {
    let mut mirror = mirror_with(&[
        noop_workout(
            1_790_398_800,
            1_790_401_500,
            "Running",
            "manual",
            json!({"avgHr": 150.0, "maxHr": 170.0}),
        ),
        noop_workout(
            1_790_438_400,
            1_790_442_600,
            "Calisthenics",
            "manual",
            json!({}),
        ),
    ])
    .await;
    // Strap samples on either side of the run, none inside it.
    mirror
        .push_hr_samples(
            SOURCE,
            IMPORTED,
            &[(1_790_398_799, 90), (1_790_401_500, 150)],
            PUSHED_AT,
        )
        .await;
    let app = app(&mirror).await;
    set_hr_max(&app, Some(193)).await;

    let run = detail_of(&app, IMPORTED, "2026-09-26T07:00:00+02:00").await;
    assert_eq!(
        run["hr"],
        json!({"basis": "workout_row", "avg_bpm": 150.0, "min_bpm": null, "max_bpm": 170.0})
    );
    assert_eq!(run.get("hr_series"), Some(&Value::Null));
    assert_eq!(run.get("hr_zones"), Some(&Value::Null));
    assert_eq!(run.get("hr_recovery"), Some(&Value::Null));
    assert_eq!(
        run["unavailable"],
        json!({"hr_series": "no_strap_samples", "hr_zones": "no_strap_samples", "hr_recovery": "no_strap_samples"})
    );

    let session = detail_of(&app, IMPORTED, "2026-09-26T18:00:00+02:00").await;
    assert_eq!(
        session["hr"],
        json!({"basis": null, "avg_bpm": null, "min_bpm": null, "max_bpm": null})
    );
    assert_eq!(session.get("hr_series"), Some(&Value::Null));
    assert_eq!(
        session["unavailable"],
        json!({"hr_series": "no_strap_samples", "hr_zones": "no_strap_samples", "hr_recovery": "no_strap_samples"})
    );
}

#[tokio::test]
async fn pace_needs_a_positive_distance() {
    let starts = [1_790_398_800, 1_790_406_000, 1_790_413_200, 1_790_420_400];
    let distances = [json!(5000.0), json!(null), json!(0.0), json!(-10.0)];
    let rows: Vec<Value> = starts
        .iter()
        .zip(&distances)
        .map(|(&start, distance)| {
            noop_workout(
                start,
                start + 1_500,
                "Running",
                "manual",
                json!({"durationS": 1500.0, "distanceM": distance}),
            )
        })
        .collect();
    let app = app(&mirror_with(&rows).await).await;

    let mut paces = Vec::new();
    for start in [
        "2026-09-26T07:00:00+02:00",
        "2026-09-26T09:00:00+02:00",
        "2026-09-26T11:00:00+02:00",
        "2026-09-26T13:00:00+02:00",
    ] {
        paces.push(detail_of(&app, IMPORTED, start).await["avg_pace_s_per_km"].clone());
    }
    // 25 minutes over 5 km is 5:00 per km.
    assert_eq!(paces, [json!(300.0), json!(null), json!(null), json!(null)]);
}

#[tokio::test]
async fn the_listed_sport_tells_apart_workouts_sharing_a_source_and_start() {
    let app = app(&mirror_with(&[
        noop_workout(
            1_790_398_800,
            1_790_401_500,
            "Running",
            "manual",
            json!({"distanceM": 7400.0}),
        ),
        noop_workout(
            1_790_398_800,
            1_790_402_400,
            "Cycling",
            "manual",
            json!({"distanceM": 20000.0}),
        ),
    ])
    .await)
    .await;
    let listed = workouts_on(&app, "2026-09-26").await["noop_workouts"].clone();
    let mut listed: Vec<(&str, &str, &str)> = listed
        .as_array()
        .unwrap()
        .iter()
        .map(|workout| {
            (
                workout["source"].as_str().unwrap(),
                workout["start"].as_str().unwrap(),
                workout["sport"].as_str().unwrap(),
            )
        })
        .collect();
    listed.sort_unstable();
    assert_eq!(
        listed,
        [
            (IMPORTED, "2026-09-26T07:00:00+02:00", "Cycling"),
            (IMPORTED, "2026-09-26T07:00:00+02:00", "Running"),
        ]
    );

    // Every listed workout opens with its listed sport.
    for (source, start, sport) in listed {
        let body = detail_of(&app, source, &format!("{start}/{sport}")).await;
        assert_eq!(body["sport"], sport);
        assert_eq!(body["start"], start);
    }

    // Without the sport the choice is ambiguous, so none is picked.
    let (status, body) = detail(&app, IMPORTED, "2026-09-26T07:00:00+02:00", Some(API_KEY)).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(
        body,
        json!({
            "error": "2 NOOP Workouts from 'my-whoop' start at 2026-09-26T07:00:00+02:00 \
                      (Cycling, Running); pass the listed sport to choose one",
            "status": 409,
        })
    );

    // The sport matches whatever its case; another sport matches nothing.
    for sport in ["running", "RUNNING"] {
        let body = detail_of(
            &app,
            IMPORTED,
            &format!("2026-09-26T07:00:00+02:00/{sport}"),
        )
        .await;
        assert_eq!(body["sport"], "Running", "{sport}");
    }
    let (status, body) = detail(
        &app,
        IMPORTED,
        "2026-09-26T07:00:00+02:00/Walking",
        Some(API_KEY),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(
        body["error"],
        "No Walking NOOP Workout from 'my-whoop' starts at 2026-09-26T07:00:00+02:00"
    );
}

#[tokio::test]
async fn an_exact_sport_wins_over_one_differing_only_in_case() {
    // Zero-length rows never overlap, so NOOP keeps both although their sports match.
    let app = app(&mirror_with(&[
        noop_workout(1_790_398_800, 1_790_398_800, "Running", "manual", json!({})),
        noop_workout(1_790_398_800, 1_790_398_800, "running", "manual", json!({})),
    ])
    .await)
    .await;

    for sport in ["Running", "running"] {
        let body = detail_of(
            &app,
            IMPORTED,
            &format!("2026-09-26T07:00:00+02:00/{sport}"),
        )
        .await;
        assert_eq!(body["sport"], sport);
    }
    let (status, body) = detail(
        &app,
        IMPORTED,
        "2026-09-26T07:00:00+02:00/RUNNING",
        Some(API_KEY),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(
        body["error"],
        "2 NOOP Workouts from 'my-whoop' start at 2026-09-26T07:00:00+02:00 (Running, running); \
         pass the listed sport to choose one"
    );
}

#[tokio::test]
async fn a_unique_workout_opens_with_or_without_its_sport() {
    let app = app(&reference_run::mirror().await).await;
    let without = detail_of(&app, IMPORTED, reference_run::START).await;
    let with = detail_of(&app, IMPORTED, &format!("{}/Running", reference_run::START)).await;
    assert_eq!(with, without);

    for (sport, shown) in [("%0A", "\n"), ("Run%09ning", "Run\tning")] {
        let (status, body) = detail(
            &app,
            IMPORTED,
            &format!("{}/{sport}", reference_run::START),
            Some(API_KEY),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{sport}: {body}");
        assert_eq!(
            body["error"],
            format!(
                "Invalid sport {shown:?}: must be a NOOP Workout sport as workouts_on_day lists it"
            )
        );
    }
}

/// The API over a mirror holding one strap-recorded Running workout from 07:00 on 2026-09-26
/// lasting `duration_s`, with strap samples at `(offset_s, bpm)` from its start, and the Profile's
/// Max Heart Rate set to `hr_max_bpm`.
async fn app_with_run(duration_s: i64, samples: &[(i64, i64)], hr_max_bpm: Option<i64>) -> Router {
    let start = 1_790_398_800;
    let mut mirror = mirror_with(&[noop_workout(
        start,
        start + duration_s,
        "Running",
        "manual",
        json!({}),
    )])
    .await;
    let samples: Vec<_> = samples
        .iter()
        .map(|&(offset, bpm)| (start + offset, bpm))
        .collect();
    mirror
        .push_hr_samples(SOURCE, IMPORTED, &samples, PUSHED_AT)
        .await;
    let app = app(&mirror).await;
    set_hr_max(&app, hr_max_bpm).await;
    app
}

/// Each zone's `(minutes, percent)`, Zone 1 first.
fn zone_split(body: &Value) -> Vec<(f64, f64)> {
    body["hr_zones"]["zones"]
        .as_array()
        .unwrap_or_else(|| panic!("no zones in {body}"))
        .iter()
        .map(|zone| {
            (
                zone["minutes"].as_f64().unwrap(),
                zone["percent"].as_f64().unwrap(),
            )
        })
        .collect()
}

fn assert_split_close(actual: &[(f64, f64)], expected: &[(f64, f64)]) {
    assert_eq!(actual.len(), expected.len(), "{actual:?}");
    for (actual, expected) in actual.iter().zip(expected) {
        assert!(
            (actual.0 - expected.0).abs() < 1e-9 && (actual.1 - expected.1).abs() < 1e-9,
            "{actual:?} != {expected:?}"
        );
    }
}

#[tokio::test]
async fn zone_lower_edges_are_inclusive_and_unrounded() {
    let second = 1.0 / 60.0;
    for (hr_max, bpm, zone_seconds) in [
        // Edges at exactly 100 / 120 / 140 / 160 / 180 bpm belong to the zone above them.
        (
            200,
            vec![99, 100, 119, 120, 139, 140, 159, 160, 179, 180, 200, 230],
            [2.0, 2.0, 2.0, 2.0, 3.0],
        ),
        // Edges at 96.5 / 115.8 / 135.1 / 154.4 / 173.7 bpm: 135 and 154 stay below 135.1 and
        // 154.4 although they round to them.
        (
            193,
            vec![96, 97, 115, 116, 135, 136, 154, 155, 173, 174],
            [2.0, 2.0, 2.0, 2.0, 1.0],
        ),
    ] {
        // One sample a second: the first is below Zone 1 and left out of the percentages.
        let samples: Vec<_> = (0..).zip(bpm.iter().copied()).collect();
        let app = app_with_run(bpm.len() as i64, &samples, Some(hr_max)).await;

        let body = detail_of(&app, IMPORTED, "2026-09-26T07:00:00+02:00").await;

        assert_eq!(body["hr_zones"]["hrmax_used"], hr_max);
        assert_close(&body["hr_zones"]["below_zone_min"], second);
        let zoned: f64 = zone_seconds.iter().sum();
        let expected: Vec<_> = zone_seconds
            .iter()
            .map(|seconds| (seconds * second, seconds / zoned * 100.0))
            .collect();
        assert_split_close(&zone_split(&body), &expected);
    }
}

#[tokio::test]
async fn a_sample_is_credited_until_the_next_one_but_at_most_the_median_interval() {
    // Gaps of 5, 5, 5, 1 and 60 s inside the 80 s workout: the median interval is 5 s. A sample at
    // the end is outside the workout and neither zoned nor the "next" sample of the last one.
    let samples = [
        (0, 150),
        (5, 150),
        (10, 150),
        (15, 150),
        (16, 190),
        (76, 190),
        (80, 110),
    ];
    let app = app_with_run(80, &samples, Some(200)).await;

    let body = detail_of(&app, IMPORTED, "2026-09-26T07:00:00+02:00").await;

    // Zone 3: 5 + 5 + 5 + 1 s. Zone 5: the 60 s gap capped at 5 s, and the last sample's 5 s.
    assert_split_close(
        &zone_split(&body),
        &[
            (0.0, 0.0),
            (0.0, 0.0),
            (16.0 / 60.0, 16.0 / 26.0 * 100.0),
            (0.0, 0.0),
            (10.0 / 60.0, 10.0 / 26.0 * 100.0),
        ],
    );
    assert_eq!(body["hr_zones"]["below_zone_min"], 0.0);
}

#[tokio::test]
async fn zones_need_strap_samples_in_the_workout() {
    // Samples on either side of the workout, none inside it.
    let app = app_with_run(600, &[(-1, 150), (600, 150)], Some(193)).await;

    let body = detail_of(&app, IMPORTED, "2026-09-26T07:00:00+02:00").await;

    assert_eq!(body.get("hr_zones"), Some(&Value::Null));
    assert_eq!(
        body["unavailable"],
        json!({"hr_series": "no_strap_samples", "hr_zones": "no_strap_samples", "hr_recovery": "no_strap_samples"})
    );
}

#[tokio::test]
async fn a_changed_max_heart_rate_rezones_the_next_read() {
    let app = app(&reference_run::mirror().await).await;
    let lower_edges = |body: &Value| -> Vec<i64> {
        body["hr_zones"]["zones"]
            .as_array()
            .unwrap()
            .iter()
            .map(|zone| zone["lower_bpm"].as_i64().unwrap())
            .collect()
    };

    set_hr_max(&app, Some(193)).await;
    let at_193 = detail_of(&app, IMPORTED, reference_run::START).await;
    assert_eq!(lower_edges(&at_193), [97, 116, 136, 155, 174]);

    set_hr_max(&app, Some(200)).await;
    let at_200 = detail_of(&app, IMPORTED, reference_run::START).await;
    assert_eq!(at_200["hr_zones"]["hrmax_used"], 200);
    assert_eq!(lower_edges(&at_200), [100, 120, 140, 160, 180]);
    assert_eq!(at_200["hr_zones"]["zones"][4]["upper_bpm"], 200);
    assert_ne!(zone_split(&at_200), zone_split(&at_193));

    set_hr_max(&app, None).await;
    let cleared = detail_of(&app, IMPORTED, reference_run::START).await;
    assert_eq!(cleared.get("hr_zones"), Some(&Value::Null));
    assert_eq!(
        cleared["unavailable"],
        json!({"hr_zones": "hr_max_not_configured", "hr_recovery": "hr_max_not_configured"})
    );
}

/// 07:00-07:30 on the 26th: a synthetic run for Heart Rate Recovery.
const RUN_START: &str = "2026-09-26T07:00:00+02:00";
const RUN_DURATION_S: i64 = 1_800;

/// `hr_recovery` and `unavailable` of the synthetic run with strap `(seconds from its end, bpm)`
/// samples and a Max Heart Rate of 200, so 70 % is 140 bpm.
async fn recovery_of(samples: &[(i64, i64)]) -> (Value, Value) {
    let from_start: Vec<_> = samples
        .iter()
        .map(|&(offset, bpm)| (RUN_DURATION_S + offset, bpm))
        .collect();
    let app = app_with_run(RUN_DURATION_S, &from_start, Some(200)).await;
    let body = detail_of(&app, IMPORTED, RUN_START).await;
    (body["hr_recovery"].clone(), body["unavailable"].clone())
}

/// `bpm` every `step` seconds from `from` through `to`, as seconds from the run's end.
fn steady(from: i64, to: i64, step: usize, bpm: i64) -> Vec<(i64, i64)> {
    (from..=to)
        .step_by(step)
        .map(|offset| (offset, bpm))
        .collect()
}

#[tokio::test]
async fn recovery_needs_120_s_of_continuous_hard_effort_in_the_last_5_minutes() {
    // 230 s at exactly 70 % of Max Heart Rate, samples 10 s apart, until the end.
    let (recovery, unavailable) = recovery_of(&steady(-230, 0, 10, 140)).await;
    assert_eq!(unavailable, json!({}));
    assert_eq!(recovery["end_hr_bpm"], 140);

    // One bpm under 70 % is not hard enough.
    let (recovery, unavailable) = recovery_of(&steady(-230, 0, 10, 139)).await;
    assert_eq!(recovery, Value::Null);
    assert_eq!(unavailable, json!({"hr_recovery": "not_sustained"}));

    // An 11 s gap splits the effort into 110 s and 109 s, neither long enough.
    let gapped = [steady(-230, -120, 10, 160), steady(-109, 0, 10, 160)].concat();
    let (recovery, unavailable) = recovery_of(&gapped).await;
    assert_eq!(recovery, Value::Null);
    assert_eq!(unavailable, json!({"hr_recovery": "not_sustained"}));

    // Effort counts only from 5 minutes before the end: 238 s, of which the last 119 s are
    // inside, then an easy finish.
    let cooled_down = [steady(-420, -182, 1, 180), steady(-181, 0, 1, 100)].concat();
    let (recovery, unavailable) = recovery_of(&cooled_down).await;
    assert_eq!(recovery, Value::Null);
    assert_eq!(unavailable, json!({"hr_recovery": "not_sustained"}));
}

#[tokio::test]
async fn end_hr_is_the_highest_of_at_least_3_samples_in_the_final_30_s() {
    let effort = steady(-230, -40, 10, 150);

    // The session peak 31 s before the end and a sample after it are outside the final 30 s.
    let samples = [
        effort.clone(),
        vec![(-31, 199), (-30, 170), (-15, 175), (0, 160), (1, 190)],
    ]
    .concat();
    let (recovery, unavailable) = recovery_of(&samples).await;
    assert_eq!(unavailable, json!({}));
    assert_eq!(
        recovery,
        json!({"end_hr_bpm": 175, "at_1_min": null, "at_2_min": null, "at_5_min": null})
    );

    let sparse = [effort, vec![(-30, 170), (0, 160), (1, 190)]].concat();
    let (recovery, unavailable) = recovery_of(&sparse).await;
    assert_eq!(recovery, Value::Null);
    assert_eq!(unavailable, json!({"hr_recovery": "no_end_samples"}));
}

#[tokio::test]
async fn each_mark_is_the_median_within_15_s_or_null_without_3_samples() {
    let samples = [
        steady(-230, -40, 10, 150),
        vec![(-20, 180), (-10, 183), (0, 181)],
        // 1 minute: three samples inside ±15 s, two just outside.
        vec![(44, 100), (45, 150), (60, 154), (75, 153), (76, 90)],
        // 2 minutes: the median of 139, 140, 141 and 145 is 140.5, which rounds up.
        vec![(110, 140), (115, 141), (125, 139), (130, 145)],
        // The strap stops before the 5-minute mark; nothing is interpolated.
    ]
    .concat();

    let (recovery, unavailable) = recovery_of(&samples).await;
    assert_eq!(unavailable, json!({}));
    assert_eq!(
        recovery,
        json!({
            "end_hr_bpm": 183,
            "at_1_min": {"hr_bpm": 153, "drop_bpm": 30},
            "at_2_min": {"hr_bpm": 141, "drop_bpm": 42},
            "at_5_min": null,
        })
    );
}

#[tokio::test]
async fn recovery_ignores_samples_outside_30_to_250_bpm() {
    let samples = [
        // A 20 bpm dropout inside the effort does not break it.
        steady(-230, -140, 10, 150),
        vec![(-135, 20)],
        steady(-130, -40, 10, 150),
        // 251 bpm is not the end HR.
        vec![(-25, 180), (-20, 251), (-10, 183), (0, 181)],
        // A 29 bpm reading does not pull the 1-minute median down to 151.
        vec![(55, 150), (58, 29), (60, 152), (65, 154)],
        // 30 and 250 bpm are kept.
        vec![(115, 30), (120, 250), (125, 140)],
        // With 251 bpm ignored, the 5-minute mark has only two samples.
        vec![(295, 140), (300, 251), (305, 140)],
    ]
    .concat();

    let (recovery, unavailable) = recovery_of(&samples).await;
    assert_eq!(unavailable, json!({}));
    assert_eq!(
        recovery,
        json!({
            "end_hr_bpm": 183,
            "at_1_min": {"hr_bpm": 152, "drop_bpm": 31},
            "at_2_min": {"hr_bpm": 140, "drop_bpm": 43},
            "at_5_min": null,
        })
    );
}

#[tokio::test]
async fn the_5_minute_mark_reads_samples_until_315_s_after_the_end() {
    let samples = [
        steady(-230, 0, 10, 150),
        vec![(300, 130), (310, 131), (315, 132)],
    ]
    .concat();

    let (recovery, _) = recovery_of(&samples).await;
    assert_eq!(recovery["at_5_min"], json!({"hr_bpm": 131, "drop_bpm": 19}));
}
