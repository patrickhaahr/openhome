//! NOOP Workout Detail: one NOOP Workout, opened by the `source` and `start` that Workouts on a
//! Training Day lists for it, with heart rate taken from the strap's samples.
//!
//! Selection runs against the merged NOOP Workout list of the workout's Training Day, so a row
//! that NOOP folds into another one, or drops as a detected shadow, cannot be opened. Heart rate
//! always comes from the strap's imported namespace, whatever namespace recorded the workout: it is
//! the user's only heart rate sensor. The stored row's average and max are only a labelled fallback
//! when the strap has no samples for the workout.
//!
//! The route, raw samples and RR intervals never leave the API; only values computed from the
//! samples are returned.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::SqlitePool;

use crate::error::{AppError, Result};

use super::calendar::{self, CalendarDay};
use super::noop_merge::{self, WorkoutOrigin, WorkoutRow};
use super::noop_source::{self, HrSample, NoopSync};
use super::workouts::{self, WORKOUT_STREAMS, WorkoutCoverage};

/// The longest NOOP Workout whose heart rate is read. A longer one is rejected rather than
/// truncated.
pub const MAX_DURATION_S: i64 = 24 * 60 * 60;
/// Seconds the strap sample read extends past the workout's end: the 5-minute Heart Rate Recovery
/// mark and its ±15 s window.
const SAMPLES_AFTER_END_S: i64 = 315;

#[derive(Debug, Serialize)]
pub struct NoopWorkoutDetail {
    pub start: String,
    pub end: String,
    pub sport: String,
    pub origin: WorkoutOrigin,
    /// The NOOP device namespace of the stored row, as Workouts on a Training Day lists it.
    pub source: String,
    /// NOOP's recorded duration.
    pub duration_min: Option<f64>,
    pub distance_m: Option<f64>,
    pub energy_kcal: Option<f64>,
    /// NOOP's derived 0-100 workout strain; a model output, not a measurement.
    pub strain_score: Option<f64>,
    /// Recorded duration over distance; null without a positive distance.
    pub avg_pace_s_per_km: Option<f64>,
    pub hr: HeartRate,
    /// Why a null block of the detail is null, keyed by the block's name. A key is present only
    /// for a block that is null.
    pub unavailable: Unavailable,
    /// Freshness and coverage of the workouts starting on the workout's Training Day.
    pub noop: NoopSync<WorkoutCoverage>,
}

/// Heart rate over the workout window `[start, end)`, in beats/min.
#[derive(Debug, Serialize)]
pub struct HeartRate {
    /// What the values were computed from; null when neither source has heart rate.
    pub basis: Option<HrBasis>,
    pub avg_bpm: Option<f64>,
    pub min_bpm: Option<f64>,
    pub max_bpm: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HrBasis {
    /// The strap's samples inside the workout window.
    StrapSamples,
    /// The stored row's average and max, unreconciled with the strap; there is no min.
    WorkoutRow,
}

/// Reasons for null blocks. Every block of this read is present so far, so it is always empty.
#[derive(Debug, Serialize)]
pub struct Unavailable {}

/// The detail of the merged NOOP Workout whose namespace is `source`, which starts at the instant
/// `start` and, when given, whose sport is `sport` in any case. NOOP keeps workouts of different
/// sports that share a namespace and start apart, so without `sport` such a collision is a
/// conflict rather than a guess.
pub async fn noop_workout_detail(
    pool: &SqlitePool,
    source: &str,
    start: DateTime<Utc>,
    sport: Option<&str>,
) -> Result<NoopWorkoutDetail> {
    let not_found = || {
        let sport = sport.map(|sport| format!("{sport} ")).unwrap_or_default();
        AppError::NotFound(format!(
            "No {sport}NOOP Workout from '{source}' starts at {}",
            calendar::iso(start)
        ))
    };
    let installation = noop_source::active_installation(pool)
        .await?
        .ok_or_else(not_found)?;
    let day = CalendarDay::of(start);
    let mut matches: Vec<WorkoutRow> =
        noop_merge::merge_workouts(workouts::noop_workout_rows(pool, &installation, day).await?)
            .into_iter()
            .filter(|row| {
                row.device_id == source
                    && row.start_ts == start.timestamp()
                    && sport.is_none_or(|sport| row.sport.to_lowercase() == sport.to_lowercase())
            })
            .collect();
    // NOOP merges overlapping rows whose sports differ only in case, so this only decides between
    // degenerate rows that do not overlap.
    if let Some(sport) = sport
        && matches.len() > 1
        && matches.iter().any(|row| row.sport == sport)
    {
        matches.retain(|row| row.sport == sport);
    }
    if matches.len() > 1 {
        let mut sports: Vec<&str> = matches.iter().map(|row| row.sport.as_str()).collect();
        sports.sort_unstable();
        return Err(AppError::Conflict(format!(
            "{} NOOP Workouts from '{source}' start at {} ({}); pass the listed sport to choose \
             one",
            matches.len(),
            calendar::iso(start),
            sports.join(", ")
        )));
    }
    let workout = matches.pop().ok_or_else(not_found)?;
    if workout.end_ts - workout.start_ts > MAX_DURATION_S {
        return Err(AppError::Unprocessable(format!(
            "The NOOP Workout from '{source}' starting at {} lasts more than 24 hours; refusing to \
             read its heart rate",
            calendar::iso(start)
        )));
    }

    let samples = noop_source::strap_hr_samples(
        pool,
        &installation,
        workout.start_ts,
        workout.end_ts + SAMPLES_AFTER_END_S,
    )
    .await?;
    let in_workout = &samples[..samples.partition_point(|sample| sample.ts < workout.end_ts)];

    let last_push_at = noop_source::last_push_at(pool, &installation, &WORKOUT_STREAMS).await?;
    let day_sync = workouts::day_sync(pool, &installation, last_push_at, day).await?;
    Ok(NoopWorkoutDetail {
        start: calendar::iso_from_unix(workout.start_ts)?,
        end: calendar::iso_from_unix(workout.end_ts)?,
        avg_pace_s_per_km: avg_pace(&workout),
        hr: heart_rate(&workout, in_workout),
        unavailable: Unavailable {},
        noop: NoopSync::synced(installation, last_push_at, day_sync),
        sport: workout.sport,
        origin: workout.origin,
        source: workout.device_id,
        duration_min: workout.duration_s.map(|seconds| seconds / 60.0),
        distance_m: workout.distance_m,
        energy_kcal: workout.energy_kcal,
        strain_score: workout.strain,
    })
}

/// Seconds per km over the recorded duration; `None` without a positive distance.
fn avg_pace(workout: &WorkoutRow) -> Option<f64> {
    let distance_km = workout.distance_m.filter(|&distance| distance > 0.0)? / 1000.0;
    Some(workout.duration_s? / distance_km)
}

/// Average, min and max over the strap samples inside the workout window, else the row's stored
/// average and max.
fn heart_rate(workout: &WorkoutRow, samples: &[HrSample]) -> HeartRate {
    if samples.is_empty() {
        let has_row_values = workout.avg_hr.is_some() || workout.max_hr.is_some();
        return HeartRate {
            basis: has_row_values.then_some(HrBasis::WorkoutRow),
            avg_bpm: workout.avg_hr,
            min_bpm: None,
            max_bpm: workout.max_hr,
        };
    }
    let bpm = || samples.iter().map(|sample| sample.bpm);
    HeartRate {
        basis: Some(HrBasis::StrapSamples),
        avg_bpm: Some(bpm().sum::<i64>() as f64 / samples.len() as f64),
        min_bpm: bpm().min().map(|bpm| bpm as f64),
        max_bpm: bpm().max().map(|bpm| bpm as f64),
    }
}
