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
    pub source: String,
    pub duration_min: Option<f64>,
    pub distance_m: Option<f64>,
    pub energy_kcal: Option<f64>,
    pub steps: Option<i64>,
    pub strain_score: Option<f64>,
    pub has_route: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route: Option<Vec<[f64; 2]>>,
    pub avg_pace_s_per_km: Option<f64>,
    pub hr: HeartRate,
    pub hr_series: Option<HeartRateSeries>,
    pub hr_zones: Option<HeartRateZones>,
    pub hr_recovery: Option<HeartRateRecovery>,
    pub unavailable: Unavailable,
    pub noop: NoopSync<WorkoutCoverage>,
}

#[derive(Debug, Serialize)]
pub struct HeartRate {
    pub basis: Option<HrBasis>,
    pub avg_bpm: Option<f64>,
    pub min_bpm: Option<f64>,
    pub max_bpm: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct HeartRateSeries {
    pub bucket_s: i64,
    pub points: Vec<HeartRatePoint>,
}

#[derive(Debug, Serialize)]
pub struct HeartRatePoint {
    pub t_s: i64,
    pub bpm: f64,
}

#[derive(Debug, Serialize)]
pub struct HeartRateZones {
    pub hrmax_used: i64,
    pub below_zone_min: f64,
    pub zones: Vec<HeartRateZone>,
}

#[derive(Debug, Serialize)]
pub struct HeartRateZone {
    pub zone: u8,
    pub lower_bpm: i64,
    pub upper_bpm: i64,
    pub minutes: f64,
    pub percent: f64,
}

#[derive(Debug, Serialize)]
pub struct HeartRateRecovery {
    pub end_hr_bpm: i64,
    pub at_1_min: Option<RecoveryMark>,
    pub at_2_min: Option<RecoveryMark>,
    pub at_5_min: Option<RecoveryMark>,
}

#[derive(Debug, Serialize)]
pub struct RecoveryMark {
    pub hr_bpm: i64,
    pub drop_bpm: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HrBasis {
    StrapSamples,
    WorkoutRow,
}

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
    HrMaxNotConfigured,
    NoStrapSamples,
    NotSustained,
    NoEndSamples,
}
