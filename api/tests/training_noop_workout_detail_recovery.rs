//! NOOP Workout Detail: heart rate recovery.

//! NOOP Workout Detail (`GET /api/training/noop-workouts/{source}/{start}`): one NOOP Workout,
//! selected by its listed `source` and `start`, with heart rate from the strap's samples. Workouts
//! and samples are pushed through the real push endpoint.

mod common;
mod training_noop_workout_detail_support;

use common::reference_run;
use common::reference_run::IMPORTED;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use training_noop_workout_detail_support::{app, app_with_run, detail_of, set_hr_max};

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
