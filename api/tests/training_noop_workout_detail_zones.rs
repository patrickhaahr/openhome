mod common;
mod training_noop_workout_detail_support;

use common::reference_run;
use common::reference_run::IMPORTED;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use training_noop_workout_detail_support::{
    app, app_with_run, assert_close, detail_of, set_hr_max,
};

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
        (
            200,
            vec![99, 100, 119, 120, 139, 140, 159, 160, 179, 180, 200, 230],
            [2.0, 2.0, 2.0, 2.0, 3.0],
        ),
        (
            193,
            vec![96, 97, 115, 116, 135, 136, 154, 155, 173, 174],
            [2.0, 2.0, 2.0, 2.0, 1.0],
        ),
    ] {
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
