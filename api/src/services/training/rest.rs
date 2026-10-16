//! Rest: NOOP's 0-100 sleep score, derived here because NOOP's push does not carry it.
//!
//! This is a port of NOOP's `AnalyticsEngine.Rest` (`Packages/StrandAnalytics/Sources/
//! StrandAnalytics/AnalyticsEngine.swift`, the metric NOOP stores as `sleep_performance`) and of
//! the two personal inputs `Strand/Data/IntelligenceEngine.swift` feeds it: the sleep need
//! (`Rest.personalizedNeedHours`) and the regularity (`VitalityEngine.sleepConsistency`). The
//! composite is the same weighted sum, constant for constant; only where its history comes from
//! differs, as documented on [`history_hours`].

use sqlx::SqlitePool;

use crate::error::Result;

use super::calendar::CalendarDay;
use super::noop_merge::ResolvedDaily;
use super::noop_source::{COMPUTED_DEVICE_ID, IMPORTED_DEVICE_ID, Installation};

/// `Rest.defaultNeedHours`: the population need before it is refined.
const DEFAULT_NEED_HOURS: f64 = 8.0;
/// `Rest.restorativeTarget`: the deep + REM share of sleep that earns full marks.
const RESTORATIVE_TARGET: f64 = 0.50;
/// `Rest.deepShareTarget`: the deep share of sleep that earns full restorative credit.
const DEEP_SHARE_TARGET: f64 = 0.13;
/// `Rest.deepFloorFactor`: how far the restorative term is scaled down when deep is absent.
const DEEP_FLOOR_FACTOR: f64 = 0.5;
/// `Rest.neutralConsistency`: the regularity scored when there is too little history.
const NEUTRAL_CONSISTENCY: f64 = 0.5;
const WEIGHT_DURATION: f64 = 0.50;
const WEIGHT_EFFICIENCY: f64 = 0.20;
const WEIGHT_RESTORATIVE: f64 = 0.20;
const WEIGHT_CONSISTENCY: f64 = 0.10;
/// `Rest.minNeedNights`: fewer scorable nights keep the population need.
const MIN_NEED_NIGHTS: usize = 7;
/// `Rest.maxNeedHours`: the cap on a personal need.
const MAX_NEED_HOURS: f64 = 9.5;
/// The adult floor of `Rest.populationNeedFloorHours`; `OpenHome` has no age, which NOOP also
/// treats as an adult.
const NEED_FLOOR_HOURS: f64 = 8.0;
/// `IntelligenceEngine` scores regularity over the latest 28 nights.
const CONSISTENCY_NIGHTS: usize = 28;
/// `computeHabitualSleep`'s efficiency for a night whose own efficiency is missing.
const FALLBACK_EFFICIENCY: f64 = 0.9;
/// How far back the nights behind the need and the regularity are read.
const HISTORY_DAYS: u64 = 90;
/// Rows read for that history; two namespaces can hold a few sessions per night.
const HISTORY_ROW_LIMIT: i64 = 1000;

/// `Rest.composite(tstSeconds:inBedSeconds:efficiency:restorativeSeconds:needHours:consistency:
/// deepSeconds:)`: 0-100, rounded to two decimals. The in-bed time is not an input of the
/// weighted sum in NOOP either.
fn composite(
    tst_seconds: f64,
    efficiency: f64,
    restorative_seconds: f64,
    need_hours: f64,
    consistency: Option<f64>,
    deep_seconds: f64,
) -> f64 {
    let clamp = |value: f64| value.clamp(0.0, 1.0);
    let need_seconds = need_hours.max(0.1) * 3600.0;
    let duration = clamp(tst_seconds / need_seconds);
    let efficiency = clamp(efficiency);
    let deep_factor = if tst_seconds > 0.0 {
        let adequacy = clamp(deep_seconds / tst_seconds / DEEP_SHARE_TARGET);
        DEEP_FLOOR_FACTOR + (1.0 - DEEP_FLOOR_FACTOR) * adequacy
    } else {
        1.0
    };
    let restorative = if tst_seconds > 0.0 {
        clamp(restorative_seconds / tst_seconds / RESTORATIVE_TARGET) * deep_factor
    } else {
        0.0
    };
    let consistency = clamp(consistency.unwrap_or(NEUTRAL_CONSISTENCY));
    let weighted = WEIGHT_DURATION * duration
        + WEIGHT_EFFICIENCY * efficiency
        + WEIGHT_RESTORATIVE * restorative
        + WEIGHT_CONSISTENCY * consistency;
    (weighted * 10_000.0).round() / 100.0
}

/// `Rest.personalizedNeedHours(nightlyHours:age:)` for an unknown age: the 75th percentile of the
/// nights' sleep, linearly interpolated, floored at the adult target and capped. Under seven
/// nights it is the population default.
fn personalized_need_hours(nightly_hours: &[f64]) -> f64 {
    let mut nights: Vec<f64> = nightly_hours.iter().copied().filter(|h| *h > 0.0).collect();
    nights.sort_by(f64::total_cmp);
    if nights.len() < MIN_NEED_NIGHTS {
        return DEFAULT_NEED_HOURS.clamp(NEED_FLOOR_HOURS, MAX_NEED_HOURS);
    }
    let position = 0.75 * (nights.len() - 1) as f64;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let low = position as usize;
    let high = (low + 1).min(nights.len() - 1);
    let quartile = nights[low] + (position - low as f64) * (nights[high] - nights[low]);
    quartile.clamp(NEED_FLOOR_HOURS, MAX_NEED_HOURS)
}

/// `VitalityEngine.sleepConsistency(nightlyHours:)`: one minus the coefficient of variation of the
/// nights' sleep (population variance), clamped to 0-1; `None` under three nights.
fn sleep_consistency(nightly_hours: &[f64]) -> Option<f64> {
    let nights: Vec<f64> = nightly_hours.iter().copied().filter(|h| *h > 0.0).collect();
    if nights.len() < 3 {
        return None;
    }
    let mean = nights.iter().sum::<f64>() / nights.len() as f64;
    if mean <= 0.0 {
        return None;
    }
    let variance = nights.iter().map(|h| (h - mean).powi(2)).sum::<f64>() / nights.len() as f64;
    Some((1.0 - variance.sqrt() / mean).clamp(0.0, 1.0))
}

/// `Rest.composite(daily:needHours:consistency:)`: the night's totals as NOOP stores them. `None`
/// without sleep or without an efficiency, as in NOOP.
fn composite_of_night(
    daily: &ResolvedDaily,
    need_hours: f64,
    consistency: Option<f64>,
) -> Option<f64> {
    let total_minutes = daily
        .total_sleep_min
        .value
        .filter(|minutes| *minutes > 0.0)?;
    let efficiency = daily.efficiency.value?;
    let deep_seconds = daily.deep_min.value.unwrap_or(0.0) * 60.0;
    let rem_seconds = daily.rem_min.value.unwrap_or(0.0) * 60.0;
    Some(composite(
        total_minutes * 60.0,
        efficiency,
        deep_seconds + rem_seconds,
        need_hours,
        consistency,
        deep_seconds,
    ))
}

/// The Rest score of the Sleep Night waking on `day`, from its daily totals and the nights before
/// it. `None` when there is no scorable sleep.
pub(super) async fn rest_score(
    pool: &SqlitePool,
    installation: &Installation,
    day: CalendarDay,
    daily: &ResolvedDaily,
) -> Result<Option<f64>> {
    if composite_of_night(daily, DEFAULT_NEED_HOURS, None).is_none() {
        return Ok(None);
    }
    let nights = history_hours(pool, installation, day).await?;
    let recent = &nights[nights.len().saturating_sub(CONSISTENCY_NIGHTS)..];
    Ok(composite_of_night(
        daily,
        personalized_need_hours(&nights),
        sleep_consistency(recent),
    ))
}

/// Hours asleep of each night in the 90 days before `day`, oldest first, as
/// `IntelligenceEngine.computeHabitualSleep` builds them: the longest session by in-bed span on
/// each local day (naps drop out), times its efficiency (0.9 when missing or out of range).
///
/// Deviations from NOOP, which reads its whole stored history at once: the window ends at `day`
/// rather than at the present, so a past night is scored on what preceded it, and it is limited to
/// 90 days; NOOP's duplicate-collapsing of timebase-shifted copies is replaced by the per-day
/// longest rule, which it also applies afterwards.
async fn history_hours(
    pool: &SqlitePool,
    installation: &Installation,
    day: CalendarDay,
) -> Result<Vec<f64>> {
    let until = day.start().timestamp();
    let from = until - HISTORY_DAYS as i64 * 86_400;
    let rows: Vec<(i64, Option<i64>, i64, Option<f64>)> = sqlx::query_as(
        "SELECT start_ts, start_ts_adjusted, end_ts, efficiency FROM sleep_session \
         WHERE source_id = ? AND device_id IN (?, ?) AND end_ts >= ? AND end_ts < ? \
         ORDER BY end_ts LIMIT ?",
    )
    .bind(&installation.source_id)
    .bind(IMPORTED_DEVICE_ID)
    .bind(COMPUTED_DEVICE_ID)
    .bind(from)
    .bind(until)
    .bind(HISTORY_ROW_LIMIT)
    .fetch_all(pool)
    .await
    .map_err(anyhow::Error::from)?;
    let mut longest = std::collections::BTreeMap::<String, (i64, f64)>::new();
    for (start_ts, adjusted, end, efficiency) in rows {
        let start = adjusted.unwrap_or(start_ts);
        if end <= start {
            continue;
        }
        let span = end - start;
        let Some(midpoint) = chrono::DateTime::from_timestamp(start + span / 2, 0) else {
            continue;
        };
        let key = CalendarDay::of(midpoint).to_string();
        if span > longest.get(&key).map_or(0, |(longest, _)| *longest) {
            let efficiency = efficiency
                .filter(|e| *e > 0.0 && *e <= 1.0)
                .unwrap_or(FALLBACK_EFFICIENCY);
            longest.insert(key, (span, span as f64 / 3600.0 * efficiency));
        }
    }
    Ok(longest.into_values().map(|(_, hours)| hours).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(actual: f64, expected: f64) {
        assert!((actual - expected).abs() < 1e-9, "{actual} != {expected}");
    }

    /// NOOP `AnalyticsEngineTests.testRestCompositePerfectNight`.
    #[test]
    fn a_perfect_night_scores_one_hundred() {
        let rest = composite(
            8.0 * 3600.0,
            1.0,
            4.0 * 3600.0,
            8.0,
            Some(1.0),
            4.0 * 3600.0,
        );
        close(rest, 100.0);
    }

    /// NOOP `testRestCompositeDurationDominatedAndClamped`: duration alone is the 0.50 weight,
    /// and sleeping past the need does not push it over 1.
    #[test]
    fn duration_is_half_the_score_and_clamps() {
        close(composite(8.0 * 3600.0, 0.0, 0.0, 8.0, Some(0.0), 0.0), 50.0);
        close(
            composite(
                12.0 * 3600.0,
                1.0,
                6.0 * 3600.0,
                8.0,
                Some(1.0),
                6.0 * 3600.0,
            ),
            100.0,
        );
    }

    /// NOOP `testRestCompositeNilConsistencyIsNeutral`: 4 h of 8 (0.5 x 50 = 25), efficiency 0.8
    /// (16), 1 h restorative of 4 h = 0.25 / 0.5 = 0.5 (10), neutral 0.5 (5) = 56. NOOP's test
    /// passes no deep split, so deep adequacy is 1; a full deep share gives the same factor.
    #[test]
    fn missing_consistency_scores_neutral() {
        let tst = 4.0 * 3600.0;
        let deep = 0.13 * tst;
        let neutral = composite(tst, 0.8, 3600.0, 8.0, None, deep);
        close(neutral, composite(tst, 0.8, 3600.0, 8.0, Some(0.5), deep));
        close(neutral, 56.0);
    }

    /// A night with no deep sleep loses half of the restorative term: 7 h at efficiency 0.9 with
    /// 1.5 h REM only: duration 0.875 x 50 = 43.75, efficiency 18, restorative
    /// (1.5/7)/0.5 = 0.428571 x 0.5 x 20 = 4.2857, consistency 0.8 x 10 = 8 -> 74.04.
    #[test]
    fn absent_deep_sleep_halves_the_restorative_term() {
        let rest = composite(7.0 * 3600.0, 0.9, 1.5 * 3600.0, 8.0, Some(0.8), 0.0);
        close(rest, 74.04);
    }

    /// NOOP `RestNeedTests`: cold start, floor, interpolated upper quartile, cap, ignored zeros.
    #[test]
    fn the_need_is_the_floored_upper_quartile() {
        close(personalized_need_hours(&[7.0, 8.0, 6.5]), 8.0);
        close(personalized_need_hours(&[5.5; 14]), 8.0);
        close(personalized_need_hours(&[11.0; 10]), 9.5);
        // Sorted 7.2 7.5 7.6 7.7 7.8 7.9 8.0 8.1 8.2 8.4: position 6.75 -> 8.0 + 0.75 x 0.1.
        let nights = [7.2, 7.8, 8.1, 7.5, 8.4, 7.9, 8.0, 7.6, 8.2, 7.7];
        assert!((personalized_need_hours(&nights) - 8.075).abs() < 1e-12);
        // A long sleeper's upper quartile lifts it: 8.5 x 8, 9.0 x 2 -> position 6.75 sits between two 8.5s.
        let long = [8.5, 8.5, 8.5, 8.5, 8.5, 8.5, 8.5, 8.5, 9.0, 9.0];
        assert!((personalized_need_hours(&long) - 8.5).abs() < 1e-12);
        // Zero and negative nights neither count nor drag.
        // Eight positive nights, sorted 7.5 .. 8.3: position 5.25 is 8.1 + 0.25 x 0.1.
        let sparse = [0.0, -1.0, 7.5, 8.0, 7.8, 8.2, 7.6, 8.1, 7.9, 8.3, 0.0];
        close(personalized_need_hours(&sparse), 8.125);
    }

    /// `VitalityEngine.sleepConsistency`: 1 - sd / mean over at least three nights.
    #[test]
    fn consistency_is_one_minus_the_coefficient_of_variation() {
        assert_eq!(sleep_consistency(&[7.0, 8.0]), None);
        close(sleep_consistency(&[8.0, 8.0, 8.0]).unwrap(), 1.0);
        // 6, 8, 10: mean 8, population variance 8/3, sd 1.63299, 1 - 0.204124 = 0.795876.
        let value = sleep_consistency(&[6.0, 8.0, 10.0]).unwrap();
        assert!((value - 0.795_876).abs() < 1e-6);
        close(sleep_consistency(&[1.0, 100.0, 1.0]).unwrap(), 0.0);
    }
}
