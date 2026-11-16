use serde::Serialize;

use super::super::noop_merge::WorkoutOrigin;
use super::super::noop_source::NoopSync;
use super::super::workouts::WorkoutCoverage;

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
    /// Steps counted over the workout.
    pub steps: Option<i64>,
    /// NOOP's derived 0-100 workout strain; a model output, not a measurement.
    pub strain_score: Option<f64>,
    /// Whether a usable GPS route was captured.
    pub has_route: bool,
    /// The captured GPS route as `[latitude, longitude]` degrees in order, thinned to at most 300
    /// points with the first and last kept. Only present when the read asked for it with
    /// `?route=true` and a route exists. Private data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route: Option<Vec<[f64; 2]>>,
    /// Recorded duration over distance; null without a positive distance.
    pub avg_pace_s_per_km: Option<f64>,
    pub hr: HeartRate,
    /// Downsampled strap heart rate over `[start, end)`; null without strap samples.
    pub hr_series: Option<HeartRateSeries>,
    /// Time in each Heart Rate Zone over `[start, end)`; null without a Max Heart Rate or strap
    /// samples.
    pub hr_zones: Option<HeartRateZones>,
    /// The heart rate drop after the workout ends; null when no Max Heart Rate is set, without
    /// strap samples, or when the end of the workout was not hard enough to measure it.
    pub hr_recovery: Option<HeartRateRecovery>,
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

/// The highest heart rate in the workout's final 30 s, and how far it dropped 1, 2 and 5 minutes
/// after the end.
#[derive(Debug, Serialize)]
pub struct HeartRateRecovery {
    pub end_hr_bpm: i64,
    pub at_1_min: Option<RecoveryMark>,
    pub at_2_min: Option<RecoveryMark>,
    pub at_5_min: Option<RecoveryMark>,
}

/// The median heart rate within 15 s of a mark; a mark with fewer than 3 samples is null rather
/// than interpolated.
#[derive(Debug, Serialize)]
pub struct RecoveryMark {
    pub hr_bpm: i64,
    /// End HR minus `hr_bpm`.
    pub drop_bpm: i64,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hr_recovery: Option<UnavailableReason>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UnavailableReason {
    /// The Profile has no Max Heart Rate, and none is ever guessed.
    HrMaxNotConfigured,
    NoStrapSamples,
    /// The last 300 s of the workout lack 120 s of continuous effort at 70 % of Max Heart Rate.
    NotSustained,
    /// Fewer than 3 samples in the workout's final 30 s.
    NoEndSamples,
}
