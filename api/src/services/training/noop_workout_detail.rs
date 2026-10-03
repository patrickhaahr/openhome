//! NOOP Workout Detail: one NOOP Workout, opened by the `source` and `start` that Workouts on a
//! Training Day lists for it, with heart rate taken from the strap's samples and split into Heart
//! Rate Zones relative to the Profile's current Max Heart Rate.
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
const BASE_BUCKET_S: i64 = 15;
const MAX_SERIES_POINTS: i64 = 300;
/// Inclusive lower edges of Heart Rate Zones 1-5 as fractions of Max Heart Rate; Zone 5 is
/// open-ended.
const ZONE_LOWER_EDGES: [f64; 5] = [0.50, 0.60, 0.70, 0.80, 0.90];
/// Sample gaps at least this long are left out of the median sample interval, as the NOOP app does.
const MAX_MEDIAN_GAP_S: i64 = 300;

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
    /// Downsampled strap heart rate over `[start, end)`; null without strap samples.
    pub hr_series: Option<HeartRateSeries>,
    /// Time in each Heart Rate Zone over `[start, end)`; null without a Max Heart Rate or strap
    /// samples.
    pub hr_zones: Option<HeartRateZones>,
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

/// Mean heart rate in workout-relative buckets; empty buckets are omitted.
#[derive(Debug, Serialize)]
pub struct HeartRateSeries {
    /// Smallest multiple of 15 seconds that keeps the workout at at most 300 buckets.
    pub bucket_s: i64,
    pub points: Vec<HeartRatePoint>,
}

#[derive(Debug, Serialize)]
pub struct HeartRatePoint {
    /// Bucket start's offset in seconds from the workout start.
    pub t_s: i64,
    pub bpm: f64,
}

/// Time in the five Heart Rate Zones, computed with the NOOP app's algorithm so the split matches
/// the app's for the same Max Heart Rate.
#[derive(Debug, Serialize)]
pub struct HeartRateZones {
    /// The Profile's Max Heart Rate the zones are relative to, in beats/min.
    pub hrmax_used: i64,
    /// Minutes below Zone 1 (under 50 % of Max Heart Rate), excluded from the percentages.
    pub below_zone_min: f64,
    /// Zones 1-5 in order.
    pub zones: Vec<HeartRateZone>,
}

#[derive(Debug, Serialize)]
pub struct HeartRateZone {
    pub zone: u8,
    /// Lowest whole bpm in the zone.
    pub lower_bpm: i64,
    /// Highest whole bpm in the zone; Max Heart Rate for the open-ended Zone 5.
    pub upper_bpm: i64,
    pub minutes: f64,
    /// Share of the time spent in Zones 1-5; 0 when there is none.
    pub percent: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HrBasis {
    /// The strap's samples inside the workout window.
    StrapSamples,
    /// The stored row's average and max, unreconciled with the strap; there is no min.
    WorkoutRow,
}

/// Reasons for null blocks, omitted when the block is available.
#[derive(Debug, Serialize)]
pub struct Unavailable {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hr_series: Option<UnavailableReason>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hr_zones: Option<UnavailableReason>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UnavailableReason {
    /// The Profile has no Max Heart Rate, and none is ever guessed.
    HrMaxNotConfigured,
    NoStrapSamples,
}

/// The detail of the merged NOOP Workout whose namespace is `source`, which starts at the instant
/// `start` and, when given, whose sport is `sport` in any case. NOOP keeps workouts of different
/// sports that share a namespace and start apart, so without `sport` such a collision is a
/// conflict rather than a guess. Heart Rate Zones use the Profile's Max Heart Rate in `db` at the
/// time of the read.
pub async fn noop_workout_detail(
    db: &SqlitePool,
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
    let hr_series = heart_rate_series(&workout, in_workout);
    let hr_max = profile_hr_max(db).await?;
    let hr_zones = match hr_max {
        Some(hr_max) if !in_workout.is_empty() => Ok(heart_rate_zones(hr_max, in_workout)),
        Some(_) => Err(UnavailableReason::NoStrapSamples),
        None => Err(UnavailableReason::HrMaxNotConfigured),
    };
    let unavailable = Unavailable {
        hr_series: hr_series
            .is_none()
            .then_some(UnavailableReason::NoStrapSamples),
        hr_zones: hr_zones.as_ref().err().copied(),
    };
    Ok(NoopWorkoutDetail {
        start: calendar::iso_from_unix(workout.start_ts)?,
        end: calendar::iso_from_unix(workout.end_ts)?,
        avg_pace_s_per_km: avg_pace(&workout),
        hr: heart_rate(&workout, in_workout),
        hr_series,
        hr_zones: hr_zones.ok(),
        unavailable,
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

/// Samples are sorted by timestamp and restricted to `[start, end)` by the caller.
fn heart_rate_series(workout: &WorkoutRow, samples: &[HrSample]) -> Option<HeartRateSeries> {
    if samples.is_empty() {
        return None;
    }
    let duration_s = workout.end_ts - workout.start_ts;
    let base_span_s = BASE_BUCKET_S * MAX_SERIES_POINTS;
    let bucket_s = BASE_BUCKET_S * ((duration_s + base_span_s - 1) / base_span_s).max(1);
    let bucket = |sample: &HrSample| (sample.ts - workout.start_ts) / bucket_s;
    let points = samples
        .chunk_by(|a, b| bucket(a) == bucket(b))
        .map(|samples| HeartRatePoint {
            t_s: bucket(&samples[0]) * bucket_s,
            bpm: samples.iter().map(|sample| sample.bpm).sum::<i64>() as f64 / samples.len() as f64,
        })
        .collect();
    Some(HeartRateSeries { bucket_s, points })
}

/// The Profile's Max Heart Rate; `None` when the Profile or the value is not set.
async fn profile_hr_max(db: &SqlitePool) -> Result<Option<i64>> {
    let hr_max = sqlx::query_scalar!("SELECT hr_max_bpm FROM profile WHERE id = 1")
        .fetch_optional(db)
        .await
        .map_err(|e| {
            AppError::Internal(anyhow::anyhow!("Failed to read the Max Heart Rate: {e}"))
        })?;
    Ok(hr_max.flatten())
}

/// Time in each zone, ported from the NOOP app's `HRZones`. Each sample is credited with the time
/// until the next one, capped at the median sample interval; the last sample gets the median.
/// Samples are sorted by timestamp, distinct, non-empty and restricted to `[start, end)`.
fn heart_rate_zones(hr_max: i64, samples: &[HrSample]) -> HeartRateZones {
    let thresholds = ZONE_LOWER_EDGES.map(|edge| edge * hr_max as f64);
    let median_s = median_interval_s(samples);
    let mut seconds_in = [0.0; 5];
    let mut below_s = 0.0;
    for (index, sample) in samples.iter().enumerate() {
        let credit_s = samples.get(index + 1).map_or(median_s, |next| {
            ((next.ts - sample.ts) as f64).min(median_s)
        });
        match thresholds
            .iter()
            .rposition(|&threshold| sample.bpm as f64 >= threshold)
        {
            Some(zone) => seconds_in[zone] += credit_s,
            None => below_s += credit_s,
        }
    }
    let zoned_s: f64 = seconds_in.iter().sum();
    let lower_bpm = thresholds.map(|threshold| threshold.ceil() as i64);
    let zones = (0..5)
        .map(|zone| HeartRateZone {
            zone: zone as u8 + 1,
            lower_bpm: lower_bpm[zone],
            upper_bpm: lower_bpm.get(zone + 1).map_or(hr_max, |next| next - 1),
            minutes: seconds_in[zone] / 60.0,
            percent: if zoned_s > 0.0 {
                seconds_in[zone] / zoned_s * 100.0
            } else {
                0.0
            },
        })
        .collect();
    HeartRateZones {
        hrmax_used: hr_max,
        below_zone_min: below_s / 60.0,
        zones,
    }
}

/// The upper median of the gaps shorter than [`MAX_MEDIAN_GAP_S`], at least 1 s; 1 s without such
/// a gap.
fn median_interval_s(samples: &[HrSample]) -> f64 {
    let mut gaps: Vec<i64> = samples
        .windows(2)
        .map(|pair| pair[1].ts - pair[0].ts)
        .filter(|&gap| gap > 0 && gap < MAX_MEDIAN_GAP_S)
        .collect();
    gaps.sort_unstable();
    gaps.get(gaps.len() / 2)
        .map_or(1.0, |&gap| gap.max(1) as f64)
}
