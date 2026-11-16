use sqlx::SqlitePool;

use crate::error::{AppError, Result};

use super::super::noop_merge::WorkoutRow;
use super::super::noop_source::HrSample;

use super::{
    BASE_BUCKET_S, ELIGIBILITY_FRACTION_OF_HR_MAX, ELIGIBILITY_LOOKBACK_S, END_WINDOW_S, HeartRate,
    HeartRatePoint, HeartRateRecovery, HeartRateSeries, HeartRateZone, HeartRateZones, HrBasis,
    MARK_TOLERANCE_S, MAX_CONTINUOUS_GAP_S, MAX_MEDIAN_GAP_S, MAX_SERIES_POINTS,
    MIN_READING_SAMPLES, MIN_SUSTAINED_S, RECOVERY_BPM_RANGE, RecoveryMark, UnavailableReason,
    ZONE_LOWER_EDGES,
};

/// Seconds per km over the recorded duration; `None` without a positive distance.
pub(super) fn avg_pace(workout: &WorkoutRow) -> Option<f64> {
    let distance_km = workout.distance_m.filter(|&distance| distance > 0.0)? / 1000.0;
    Some(workout.duration_s? / distance_km)
}

/// Average, min and max over the strap samples inside the workout window, else the row's stored
/// average and max.
pub(super) fn heart_rate(workout: &WorkoutRow, samples: &[HrSample]) -> HeartRate {
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
pub(super) fn heart_rate_series(
    workout: &WorkoutRow,
    samples: &[HrSample],
) -> Option<HeartRateSeries> {
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
pub(super) async fn profile_hr_max(db: &SqlitePool) -> Result<Option<i64>> {
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
pub(super) fn heart_rate_zones(hr_max: i64, samples: &[HrSample]) -> HeartRateZones {
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

/// Heart Rate Recovery as the NOOP app computes it. `samples` are sorted by timestamp and span
/// `[start, end + 315 s]`.
pub(super) fn heart_rate_recovery(
    workout: &WorkoutRow,
    samples: &[HrSample],
    hr_max_bpm: i64,
) -> std::result::Result<HeartRateRecovery, UnavailableReason> {
    let end = workout.end_ts;
    let lookback_start = workout.start_ts.max(end - ELIGIBILITY_LOOKBACK_S);
    let samples: Vec<&HrSample> = samples
        .iter()
        .filter(|sample| sample.ts >= lookback_start && RECOVERY_BPM_RANGE.contains(&sample.bpm))
        .collect();
    let until_end = &samples[..samples.partition_point(|sample| sample.ts <= end)];

    let threshold = hr_max_bpm as f64 * ELIGIBILITY_FRACTION_OF_HR_MAX;
    if sustained_s(until_end, threshold) < MIN_SUSTAINED_S {
        return Err(UnavailableReason::NotSustained);
    }
    let end_window = until_end
        .iter()
        .filter(|sample| sample.ts >= end - END_WINDOW_S);
    if end_window.clone().count() < MIN_READING_SAMPLES {
        return Err(UnavailableReason::NoEndSamples);
    }
    let end_hr_bpm = end_window
        .map(|sample| sample.bpm)
        .max()
        .unwrap_or_default();

    let mark = |minutes: i64| {
        let at = end + minutes * 60;
        let mut bpm: Vec<i64> = samples
            .iter()
            .filter(|sample| (sample.ts - at).abs() <= MARK_TOLERANCE_S)
            .map(|sample| sample.bpm)
            .collect();
        if bpm.len() < MIN_READING_SAMPLES {
            return None;
        }
        bpm.sort_unstable();
        let middle = bpm.len() / 2;
        let hr_bpm = if bpm.len().is_multiple_of(2) {
            ((bpm[middle - 1] + bpm[middle]) as f64 / 2.0).round() as i64
        } else {
            bpm[middle]
        };
        Some(RecoveryMark {
            hr_bpm,
            drop_bpm: end_hr_bpm - hr_bpm,
        })
    };
    Ok(HeartRateRecovery {
        end_hr_bpm,
        at_1_min: mark(1),
        at_2_min: mark(2),
        at_5_min: mark(5),
    })
}

/// The longest run of seconds at or above `threshold`, where each sample at or above it is
/// credited with the time to the next one when that is at most 10 s away.
fn sustained_s(samples: &[&HrSample], threshold: f64) -> i64 {
    let mut current = 0;
    let mut longest = 0;
    for pair in samples.windows(2) {
        let gap = pair[1].ts - pair[0].ts;
        if gap > MAX_CONTINUOUS_GAP_S || (pair[0].bpm as f64) < threshold {
            current = 0;
        } else {
            current += gap;
            longest = longest.max(current);
        }
    }
    longest
}
