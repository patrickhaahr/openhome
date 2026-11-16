//! NOOP Workout Detail: one NOOP Workout, opened by the `source` and `start` that Workouts on a
//! Training Day lists for it, with heart rate taken from the strap's samples, split into Heart Rate
//! Zones relative to the Profile's current Max Heart Rate, and its Heart Rate Recovery.
//!
//! Selection runs against the merged NOOP Workout list of the workout's Training Day, so a row
//! that NOOP folds into another one, or drops as a detected shadow, cannot be opened. Heart rate
//! always comes from the strap's imported namespace, whatever namespace recorded the workout: it is
//! the user's only heart rate sensor. The stored row's average and max are only a labelled fallback
//! when the strap has no samples for the workout.
//!
//! Heart Rate Recovery ports the NOOP app's `HeartRateRecovery` so the API reports the drops the
//! app shows. It needs the Profile's Max Heart Rate, which is never inferred.
//!
//! Raw samples and RR intervals never leave the API; only values computed from the samples are
//! returned. The route is returned decoded and thinned (see `route`), to authenticated clients
//! only; it is private household data and is never logged.

use chrono::{DateTime, Utc};
use sqlx::SqlitePool;

use crate::error::{AppError, Result};

use super::calendar::{self, CalendarDay};
use super::noop_merge::{self, WorkoutRow};
use super::noop_source::{self, NoopSync};
use super::route;
use super::workouts::{self, WORKOUT_STREAMS};

mod heart_rate;
mod types;

use heart_rate::{
    avg_pace, heart_rate, heart_rate_recovery, heart_rate_series, heart_rate_zones, profile_hr_max,
};
pub use types::{
    HeartRate, HeartRatePoint, HeartRateRecovery, HeartRateSeries, HeartRateZone, HeartRateZones,
    HrBasis, NoopWorkoutDetail, RecoveryMark, Unavailable, UnavailableReason,
};

/// The longest NOOP Workout whose heart rate is read. A longer one is rejected rather than
/// truncated.
pub const MAX_DURATION_S: i64 = 24 * 60 * 60;
/// Seconds the strap sample read extends past the workout's end: the 5-minute Heart Rate Recovery
/// mark and its ±15 s window.
const SAMPLES_AFTER_END_S: i64 = 315;
const BASE_BUCKET_S: i64 = 15;
/// Heart rate outside this range is ignored for Heart Rate Recovery.
const RECOVERY_BPM_RANGE: std::ops::RangeInclusive<i64> = 30..=250;
/// Recovery is meaningful only after hard effort that lasts until the end: within the last
/// 300 s, 120 s continuously at or above 70 % of Max Heart Rate, where consecutive samples at
/// most 10 s apart are continuous.
const ELIGIBILITY_LOOKBACK_S: i64 = 300;
const ELIGIBILITY_FRACTION_OF_HR_MAX: f64 = 0.70;
const MIN_SUSTAINED_S: i64 = 120;
const MAX_CONTINUOUS_GAP_S: i64 = 10;
/// End HR is the highest heart rate in the workout's final 30 s.
const END_WINDOW_S: i64 = 30;
/// A mark is the median heart rate within 15 s of it.
const MARK_TOLERANCE_S: i64 = 15;
/// Samples an end HR or a mark needs.
const MIN_READING_SAMPLES: usize = 3;
const MAX_SERIES_POINTS: i64 = 300;
/// Inclusive lower edges of Heart Rate Zones 1-5 as fractions of Max Heart Rate; Zone 5 is
/// open-ended.
const ZONE_LOWER_EDGES: [f64; 5] = [0.50, 0.60, 0.70, 0.80, 0.90];
/// Sample gaps at least this long are left out of the median sample interval, as the NOOP app does.
const MAX_MEDIAN_GAP_S: i64 = 300;

/// The detail of the merged NOOP Workout whose namespace is `source`, which starts at the instant
/// `start` and, when given, whose sport is `sport` in any case. NOOP keeps workouts of different
/// sports that share a namespace and start apart, so without `sport` such a collision is a
/// conflict rather than a guess. Heart Rate Zones and Heart Rate Recovery use the Profile's Max Heart Rate in `db` at the
/// time of the read.
pub async fn noop_workout_detail(
    db: &SqlitePool,
    pool: &SqlitePool,
    source: &str,
    start: DateTime<Utc>,
    sport: Option<&str>,
    include_route: bool,
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
    let hr_series = heart_rate_series(&workout, in_workout);
    let hr_max = profile_hr_max(db).await?;
    let hr_zones = match hr_max {
        Some(hr_max) if !in_workout.is_empty() => Ok(heart_rate_zones(hr_max, in_workout)),
        Some(_) => Err(UnavailableReason::NoStrapSamples),
        None => Err(UnavailableReason::HrMaxNotConfigured),
    };
    let hr_recovery = match hr_max {
        None => Err(UnavailableReason::HrMaxNotConfigured),
        Some(_) if in_workout.is_empty() => Err(UnavailableReason::NoStrapSamples),
        Some(hr_max) => heart_rate_recovery(&workout, &samples, hr_max),
    };
    let unavailable = Unavailable {
        hr_series: hr_series
            .is_none()
            .then_some(UnavailableReason::NoStrapSamples),
        hr_zones: hr_zones.as_ref().err().copied(),
        hr_recovery: hr_recovery.as_ref().err().copied(),
    };
    let route = stored_route(pool, &installation, &workout).await?;
    Ok(NoopWorkoutDetail {
        start: calendar::iso_from_unix(workout.start_ts)?,
        end: calendar::iso_from_unix(workout.end_ts)?,
        avg_pace_s_per_km: avg_pace(&workout),
        hr: heart_rate(&workout, in_workout),
        hr_series,
        hr_zones: hr_zones.ok(),
        hr_recovery: hr_recovery.ok(),
        unavailable,
        noop: NoopSync::synced(installation, last_push_at, day_sync),
        sport: workout.sport,
        origin: workout.origin,
        source: workout.device_id,
        duration_min: workout.duration_s.map(|seconds| seconds / 60.0),
        distance_m: workout.distance_m,
        energy_kcal: workout.energy_kcal,
        steps: workout.steps,
        strain_score: workout.strain,
        has_route: route.is_some(),
        route: route.filter(|_| include_route),
    })
}

/// The workout row's decoded and thinned route. Nothing about it is logged.
async fn stored_route(
    pool: &SqlitePool,
    installation: &noop_source::Installation,
    workout: &WorkoutRow,
) -> Result<Option<Vec<[f64; 2]>>> {
    let encoded: Option<String> = sqlx::query_scalar(
        "SELECT route_polyline FROM workout \
         WHERE source_id = ? AND device_id = ? AND start_ts = ? AND sport = ?",
    )
    .bind(&installation.source_id)
    .bind(&workout.device_id)
    .bind(workout.start_ts)
    .bind(&workout.sport)
    .fetch_optional(pool)
    .await
    .map_err(anyhow::Error::from)?
    .flatten();
    Ok(encoded
        .as_deref()
        .and_then(route::decode)
        .map(|fixes| route::downsample(fixes, route::MAX_ROUTE_POINTS)))
}
