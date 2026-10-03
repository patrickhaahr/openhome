//! The reference run: the real Running NOOP Workout of 2026-09-27 (16:33:52 → 17:19:32
//! Copenhagen) and the strap's real heart rate samples from its start until 600 s after its end.
//! The NOOP app shows Heart Rate Zones and Heart Rate Recovery for it, so it pins the API's ports of
//! the app's algorithms.
#![allow(dead_code)]

use serde_json::{Value, json};

use super::mirror::{Mirror, Window, noop_workout};

/// The production installation and NOOP's strap namespaces.
pub const SOURCE: &str = "81906e30-187d-4546-8f8a-9949b82d62fa";
pub const IMPORTED: &str = "my-whoop";
pub const COMPUTED: &str = "my-whoop-noop";

pub const START_TS: i64 = 1_790_519_632;
pub const END_TS: i64 = 1_790_522_372;
/// The workout's `start` and `end` as `workouts_on_day` lists them.
pub const START: &str = "2026-09-27T16:33:52+02:00";
pub const END: &str = "2026-09-27T17:19:32+02:00";

/// NOOP's 14-day workout window ending with 2026-09-27 (local midnights).
pub const WORKOUT_WINDOW: Window = Window::WorkoutStarts(1_789_336_800, 1_790_546_400);
/// The push that brought the run: 06:00 local on 2026-09-28, after its Training Day ended.
pub const PUSHED_AT: &str = "2026-09-28T04:00:00.000Z";

/// The `workout` row exactly as NOOP stored it, started manually in NOOP. Its average and max heart
/// rate were not reconciled with the strap samples. The real route is replaced by a stand-in.
pub fn row() -> Value {
    noop_workout(
        START_TS,
        END_TS,
        "Running",
        "manual",
        json!({
            "durationS": 2739.582, "energyKcal": 817.170682229156, "avgHr": 160.0, "maxHr": 183.0,
            "strain": 58.45, "distanceM": 7169.41850045851, "routePolyline": "route-stand-in",
        }),
    )
}

/// The strap's `(ts, bpm)` samples from `START_TS` through `END_TS + 600`: 1 Hz without gaps.
pub fn samples() -> Vec<(i64, i64)> {
    include_str!("../fixtures/reference_run_hr.csv")
        .lines()
        .skip(1)
        .map(|line| {
            let (offset, bpm) = line.split_once(',').expect("t_s,bpm");
            (
                START_TS + offset.parse::<i64>().unwrap(),
                bpm.parse().unwrap(),
            )
        })
        .collect()
}

/// A mirror holding the reference run: both strap namespaces pushed the workout window, and the
/// strap pushed the run's samples.
pub async fn mirror() -> Mirror {
    let mut mirror = Mirror::empty().await;
    mirror
        .push(SOURCE, IMPORTED, WORKOUT_WINDOW, &[row()], PUSHED_AT)
        .await;
    mirror
        .push(SOURCE, COMPUTED, WORKOUT_WINDOW, &[], PUSHED_AT)
        .await;
    mirror
        .push_hr_samples(SOURCE, IMPORTED, &samples(), PUSHED_AT)
        .await;
    mirror
}
