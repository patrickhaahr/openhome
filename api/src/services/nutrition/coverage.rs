//! Micronutrient coverage over a range, for the Nutrition Summary: each vitamin and mineral of the
//! micronutrient panel per period and over the whole range, judged day by day exactly as the
//! Nutrition Day's panel judges it ([`Diary::assess`](super::micronutrients::Diary::assess)), so
//! supplement-only Upper Limits and EPA+DHA work the same way.
//!
//! - **Recommended Intake**: the mean of each logged day's percentage of its Recommended Intake.
//!   A logged day is one the summary counts as logged (Meal Logs with known energy), so a day
//!   with only Supplement Doses, or without Meal Logs, is left out rather than counted as a day
//!   of eating. A logged day whose amount is unknown (no Portion has a value and no dose lists
//!   it) is left out of that nutrient's mean: unknown is never zero. Each day uses its own age
//!   band, so a birthday inside the range is handled day by day.
//! - **Upper Limits**: the days whose amount is above any of the nutrient's Upper Limits (the
//!   Day view's caution, or a day above within a cut down), and whether the trailing 7-day
//!   average was above one on any day of the period with Meal Logs (the Day view's cut down). A
//!   window ending on a day without Meal Logs only repeats earlier days, so it is not counted,
//!   and a period without Meal Logs is never flagged; a period's first logged days can still be
//!   cut down by the days before it. Both are null for a nutrient without an Upper Limit.
//! - **Lasting shortfall**: the range's mean is below [`SHORTFALL_BELOW_PERCENT`] of the
//!   Recommended Intake over at least [`SHORTFALL_MIN_LOGGED_DAYS`] logged days with a known
//!   amount. NNR 2023 sets a recommended intake about two standard deviations above the average
//!   requirement, so the average requirement sits at roughly 70 to 85 % of it: a habitual intake
//!   below 70 % is below what most people need, not just below the target, and a week of logs is
//!   the least that shows a habit. Reaching or passing the Recommended Intake is never flagged.

use std::ops::AddAssign;

use serde::Serialize;

use super::micronutrients::references::{Group, REFERENCES, Reference};
use super::micronutrients::{Citation, DayAssessment, SOURCES, Unavailable};
use super::targets::round_to;

/// A range mean below this percentage of the Recommended Intake is a shortfall.
pub const SHORTFALL_BELOW_PERCENT: f64 = 70.0;
/// The fewest logged days with a known amount that make a shortfall lasting.
pub const SHORTFALL_MIN_LOGGED_DAYS: usize = 7;

/// One nutrient over a period or the range.
#[derive(Debug, Serialize)]
pub struct NutrientCoverage {
    pub key: &'static str,
    /// The mean over logged days with a known amount, rounded to 0.1; null without one.
    pub percent_of_recommended: Option<f64>,
    pub averaged_days: usize,
    /// Days above an Upper Limit; null for a nutrient without one.
    pub days_above_upper_limit: Option<usize>,
    /// A trailing 7-day average ending on a day with Meal Logs was above an Upper Limit; null
    /// for a nutrient without one.
    pub seven_day_average_above_upper_limit: Option<bool>,
}

/// A period's coverage of every nutrient of the panel, in the panel's order.
#[derive(Debug, Serialize)]
pub struct PeriodCoverage {
    /// Logged days evaluated against the references.
    pub logged_days: usize,
    pub nutrients: Vec<NutrientCoverage>,
}

/// One nutrient over the range, with what names it.
#[derive(Debug, Serialize)]
pub struct RangeNutrient {
    #[serde(flatten)]
    pub coverage: NutrientCoverage,
    pub name: &'static str,
    pub group: Group,
    pub unit: &'static str,
    /// An NNR adequate intake (AI), less certain than a recommended intake (RI).
    pub adequate_intake: bool,
    pub lasting_shortfall: bool,
}

/// When a range mean is a lasting shortfall.
#[derive(Debug, Serialize)]
pub struct ShortfallRule {
    pub below_percent: f64,
    pub min_logged_days: usize,
}

#[derive(Debug, Serialize)]
pub struct Coverage {
    /// Logged days evaluated against the references.
    pub logged_days: usize,
    pub shortfall: ShortfallRule,
    /// In the panel's order.
    pub nutrients: Vec<RangeNutrient>,
    pub sources: &'static [Citation],
}

/// The range's coverage, or why there is none.
#[derive(Debug, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum MicronutrientCoverage {
    Available(Box<Coverage>),
    Unavailable(Unavailable),
}

#[derive(Clone, Copy, Debug, Default)]
struct NutrientTally {
    percent_sum: f64,
    averaged_days: usize,
    days_above: usize,
    cut_down: bool,
}

/// Days added up, one tally per nutrient in the panel's order.
#[derive(Clone, Debug)]
pub(super) struct Tally {
    logged_days: usize,
    nutrients: Vec<NutrientTally>,
}

impl Default for Tally {
    fn default() -> Self {
        Self {
            logged_days: 0,
            nutrients: vec![NutrientTally::default(); REFERENCES.len()],
        }
    }
}

impl AddAssign<&Self> for Tally {
    fn add_assign(&mut self, other: &Self) {
        self.logged_days += other.logged_days;
        for (tally, other) in self.nutrients.iter_mut().zip(&other.nutrients) {
            tally.percent_sum += other.percent_sum;
            tally.averaged_days += other.averaged_days;
            tally.days_above += other.days_above;
            tally.cut_down |= other.cut_down;
        }
    }
}

impl Tally {
    /// Adds one day; `logged` when the summary counts it as a logged day.
    pub(super) fn add(&mut self, day: &DayAssessment, logged: bool) {
        self.logged_days += usize::from(logged);
        for (tally, nutrient) in self.nutrients.iter_mut().zip(&day.nutrients) {
            if let Some(percent) = nutrient.percent_of_recommended().filter(|_| logged) {
                tally.percent_sum += percent;
                tally.averaged_days += 1;
            }
            tally.days_above += usize::from(nutrient.above_upper_limit() == Some(true));
            tally.cut_down |= day.has_meal_logs && nutrient.cut_down() == Some(true);
        }
    }

    pub(super) fn period(&self) -> PeriodCoverage {
        PeriodCoverage {
            logged_days: self.logged_days,
            nutrients: self.coverages().map(|(_, _, coverage)| coverage).collect(),
        }
    }

    pub(super) fn range(&self) -> Coverage {
        Coverage {
            logged_days: self.logged_days,
            shortfall: ShortfallRule {
                below_percent: SHORTFALL_BELOW_PERCENT,
                min_logged_days: SHORTFALL_MIN_LOGGED_DAYS,
            },
            nutrients: self
                .coverages()
                .map(|(reference, tally, coverage)| RangeNutrient {
                    coverage,
                    name: reference.name,
                    group: reference.group,
                    unit: reference.unit,
                    adequate_intake: reference.adequate,
                    lasting_shortfall: tally.averaged_days >= SHORTFALL_MIN_LOGGED_DAYS
                        && tally
                            .mean()
                            .is_some_and(|mean| mean < SHORTFALL_BELOW_PERCENT),
                })
                .collect(),
            sources: SOURCES,
        }
    }

    fn coverages(
        &self,
    ) -> impl Iterator<Item = (&'static Reference, NutrientTally, NutrientCoverage)> + '_ {
        REFERENCES
            .iter()
            .zip(&self.nutrients)
            .map(|(reference, tally)| {
                let limited = !reference.upper_limits.is_empty();
                let coverage = NutrientCoverage {
                    key: reference.key,
                    percent_of_recommended: tally.mean().map(|mean| round_to(mean, 1)),
                    averaged_days: tally.averaged_days,
                    days_above_upper_limit: limited.then_some(tally.days_above),
                    seven_day_average_above_upper_limit: limited.then_some(tally.cut_down),
                };
                (reference, *tally, coverage)
            })
    }
}

impl NutrientTally {
    /// The unrounded mean percentage.
    fn mean(self) -> Option<f64> {
        (self.averaged_days > 0).then(|| self.percent_sum / self.averaged_days as f64)
    }
}
