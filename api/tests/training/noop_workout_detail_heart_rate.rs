use crate::mirror::Mirror;
use crate::mirror::noop_workout;
use crate::noop_workout_detail_support::{
    API_KEY, app, assert_close, detail_of, mirror_with, set_hr_max, workouts_on,
};
use crate::reference_run;
use crate::reference_run::COMPUTED;
use crate::reference_run::IMPORTED;
use crate::reference_run::PUSHED_AT;
use crate::reference_run::SOURCE;
use crate::reference_run::WORKOUT_WINDOW;
use crate::send_request::send_request;
use axum::http::StatusCode;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;

#[tokio::test]
async fn the_reference_run_reports_heart_rate_from_the_strap_samples() {
    let app = app(&reference_run::mirror().await).await;

    let mut body = detail_of(&app, IMPORTED, reference_run::START).await;

    assert_eq!(body["hr"]["basis"], "strap_samples");
    assert_close(&body["hr"]["avg_bpm"], 166.075912408759);
    assert_eq!(body["hr"]["min_bpm"], 84.0);
    assert_eq!(body["hr"]["max_bpm"], 185.0);
    assert_close(&body["avg_pace_s_per_km"], 382.12053039235946);
    body["hr"]["avg_bpm"] = Value::Null;
    body["avg_pace_s_per_km"] = Value::Null;
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
            "steps": null,
            "has_route": false,
            "avg_pace_s_per_km": null,
            "hr": {"basis": "strap_samples", "avg_bpm": null, "min_bpm": 84.0, "max_bpm": 185.0},
            "hr_series": null,
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
async fn heart_rate_comes_from_the_strap_whatever_namespace_recorded_the_workout() {
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
    assert_eq!(paces, [json!(300.0), json!(null), json!(null), json!(null)]);
}

#[tokio::test]
async fn a_captured_route_is_decoded_to_latitude_longitude_pairs_with_the_steps() {
    let run = noop_workout(
        1_790_438_400,
        1_790_442_000,
        "Running",
        "manual",
        json!({"durationS": 3600.0, "steps": 5812, "routePolyline": "_p~iF~ps|U_ulLnnqC"}),
    );
    let mut mirror = Mirror::empty().await;
    mirror
        .push(
            SOURCE,
            IMPORTED,
            WORKOUT_WINDOW,
            std::slice::from_ref(&run),
            PUSHED_AT,
        )
        .await;
    mirror
        .push(SOURCE, COMPUTED, WORKOUT_WINDOW, &[], PUSHED_AT)
        .await;
    let app = app(&mirror).await;
    let day = workouts_on(&app, "2026-09-26").await;
    let listed = &day["noop_workouts"][0];
    assert_eq!(listed["steps"], 5812);

    let body = detail_of(
        &app,
        listed["source"].as_str().unwrap(),
        listed["start"].as_str().unwrap(),
    )
    .await;

    assert_eq!(body["steps"], 5812);
    assert_eq!(body["has_route"], true);
    assert!(body.get("route").is_none(), "{body}");
    let (status, with_route) = send_request(
        app.clone(),
        &format!(
            "/api/training/noop-workouts/{}/{}/Running?route=true",
            listed["source"].as_str().unwrap(),
            listed["start"].as_str().unwrap()
        ),
        Some(API_KEY),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{with_route}");
    assert_eq!(
        with_route["route"],
        json!([[38.5, -120.2], [40.7, -120.95]])
    );
    let (status, body) = send_request(
        app.clone(),
        &format!(
            "/api/training/noop-workouts/{}/{}/Running?route=true",
            listed["source"].as_str().unwrap(),
            listed["start"].as_str().unwrap()
        ),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(!body.to_string().contains("38.5"), "{body}");
}
