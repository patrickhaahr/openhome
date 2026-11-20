use std::ops::AddAssign;

use serde::Serialize;

use super::micronutrients::references::{Group, REFERENCES, Reference};
use super::micronutrients::{Citation, DayAssessment, SOURCES, Unavailable};
use super::targets::round_to;

pub const SHORTFALL_BELOW_PERCENT: f64 = 70.0;
pub const SHORTFALL_MIN_LOGGED_DAYS: usize = 7;

#[derive(Debug, Serialize)]
pub struct NutrientCoverage {
    pub key: &'static str,
    pub percent_of_recommended: Option<f64>,
    pub averaged_days: usize,
    pub days_above_upper_limit: Option<usize>,
    pub seven_day_average_above_upper_limit: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct PeriodCoverage {
    pub logged_days: usize,
    pub nutrients: Vec<NutrientCoverage>,
}

#[derive(Debug, Serialize)]
pub struct RangeNutrient {
    #[serde(flatten)]
    pub coverage: NutrientCoverage,
    pub name: &'static str,
    pub group: Group,
    pub unit: &'static str,
    pub adequate_intake: bool,
    pub lasting_shortfall: bool,
}

#[derive(Debug, Serialize)]
pub struct ShortfallRule {
    pub below_percent: f64,
    pub min_logged_days: usize,
}

#[derive(Debug, Serialize)]
pub struct Coverage {
    pub logged_days: usize,
    pub shortfall: ShortfallRule,
    pub nutrients: Vec<RangeNutrient>,
    pub sources: &'static [Citation],
}

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
    pub(super) fn add(&mut self, day: &DayAssessment) {
        let logged = day.has_meal_logs;
        self.logged_days += usize::from(logged);
        for (tally, nutrient) in self.nutrients.iter_mut().zip(&day.nutrients) {
            if let Some(percent) = nutrient.percent_of_recommended().filter(|_| logged) {
                tally.percent_sum += percent;
                tally.averaged_days += 1;
            }
            tally.days_above += usize::from(nutrient.above_upper_limit() == Some(true));
            tally.cut_down |= logged && nutrient.cut_down() == Some(true);
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
    fn mean(self) -> Option<f64> {
        (self.averaged_days > 0).then(|| self.percent_sum / self.averaged_days as f64)
    }
}
