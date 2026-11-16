use super::references::{Measure, Reference, Share};
use super::{DHA, EPA, LimitStatus, RETINOL};
use crate::services::nutrition::intake::Intake;
use crate::services::nutrition::supplements::StoredDose;
use crate::services::nutrition::targets::{Missing, Sex};
use crate::services::training::calendar::CalendarDay;
use sqlx::SqlitePool;
use std::collections::HashMap;

/// The Profile values the references depend on.
#[derive(Clone, Copy, Debug)]
pub struct Person {
    pub(super) sex: Sex,
    pub(super) birth_date: CalendarDay,
}

impl Person {
    /// The Profile's sex and birth date, or which of them it lacks.
    pub async fn read(db: &SqlitePool) -> Result<Result<Self, Vec<Missing>>, sqlx::Error> {
        let row =
            sqlx::query!(r#"SELECT sex, birth_date AS "birth_date?" FROM profile WHERE id = 1"#)
                .fetch_optional(db)
                .await?;
        let (sex, birth) = row.map_or((None, None), |row| (row.sex, row.birth_date));
        let sex = sex.as_deref().and_then(Sex::parse);
        let birth_date = birth.as_deref().and_then(CalendarDay::parse);
        Ok(match (sex, birth_date) {
            (Some(sex), Some(birth_date)) => Ok(Self { sex, birth_date }),
            _ => Err([
                (sex.is_none(), Missing::Sex),
                (birth_date.is_none(), Missing::BirthDate),
            ]
            .into_iter()
            .filter_map(|(missing, what)| missing.then_some(what))
            .collect()),
        })
    }
}

fn add(left: Option<f64>, right: Option<f64>) -> Option<f64> {
    match (left, right) {
        (None, None) => None,
        (left, right) => Some(left.unwrap_or(0.0) + right.unwrap_or(0.0)),
    }
}

/// One logged day's Portions summed per parameter in SQL, with its Supplement Doses: what the
/// panel reads, without loading every Portion's values.
#[derive(Debug, Default)]
pub(super) struct DayEaten {
    /// Per parameter key: the amount from the Portions that have a value, and how many do.
    pub(super) food: HashMap<String, (f64, usize)>,
    pub(super) portions: usize,
    pub(super) doses: Vec<StoredDose>,
}

impl DayEaten {
    /// `key` from food and supplements; a Portion without a value is unknown and counted.
    fn intake(&self, key: &str) -> Intake {
        let doses: Vec<&StoredDose> = self.doses.iter().collect();
        let food = match self.food.get(key) {
            Some(&(amount, known)) => Intake::from_food(Some(amount), self.portions - known),
            None => Intake::from_food(None, self.portions),
        };
        food + Intake::of(key, &[], &doses)
    }

    /// The supplement share of `form` over `keys`, zero when none was taken.
    fn supplement_form(&self, keys: &[&str], form: &str) -> f64 {
        keys.iter()
            .filter_map(|key| self.intake(key).supplement_form(form))
            .sum()
    }
}

/// One nutrient's intake on one logged day.
#[derive(Clone, Debug)]
pub(super) struct DayIntake {
    pub(super) amount: Option<f64>,
    pub(super) food: Option<f64>,
    pub(super) supplements: Option<f64>,
    pub(super) unknown_portions: usize,
    /// The share each of the reference's Upper Limits applies to, in its order.
    pub(super) limited: Vec<Option<f64>>,
}

impl DayIntake {
    pub(super) fn of(reference: &Reference, eaten: &DayEaten) -> Self {
        let (amount, food, supplements, unknown_portions) = match reference.measure {
            Measure::Parameter(key) => {
                let intake = eaten.intake(key);
                (
                    intake.total(),
                    intake.food(),
                    intake.supplements(),
                    intake.unknown_portions(),
                )
            }
            Measure::EpaDha => {
                let (epa, dha) = (eaten.intake(EPA), eaten.intake(DHA));
                let mg = |grams: Option<f64>| grams.map(|grams| grams * 1000.0);
                (
                    mg(add(epa.total(), dha.total())),
                    mg(add(epa.food(), dha.food())),
                    mg(add(epa.supplements(), dha.supplements())),
                    epa.unknown_portions().max(dha.unknown_portions()),
                )
            }
        };
        let limited = reference
            .upper_limits
            .iter()
            .map(|limit| match limit.share {
                Share::Total => amount,
                Share::Supplements => Some(supplements.unwrap_or(0.0)),
                // Retinol in food, and retinol taken as vitamin A or listed as retinol itself.
                Share::PreformedVitaminA => add(
                    eaten.intake(RETINOL).total(),
                    eaten.intake("vitamin_a").supplement_form("retinol"),
                ),
                Share::SupplementalFolate => Some(
                    eaten.supplement_form(&["folate"], "folic_acid")
                        + eaten.supplement_form(&["folate"], "methylfolate"),
                ),
                Share::NicotinicAcid => {
                    Some(eaten.supplement_form(&["niacin_equivalent", "niacin"], "nicotinic_acid"))
                }
                Share::Nicotinamide => {
                    Some(eaten.supplement_form(&["niacin_equivalent", "niacin"], "nicotinamide"))
                }
            })
            .collect();
        Self {
            amount,
            food,
            supplements,
            unknown_portions,
            limited,
        }
    }
}

/// One Upper Limit on one day, before rounding.
#[derive(Debug)]
pub(super) struct LimitDay {
    pub(super) limit: f64,
    /// The day's amount of the share the limit applies to.
    pub(super) amount: Option<f64>,
    pub(super) average: Option<f64>,
    pub(super) averaged_days: usize,
    pub(super) status: LimitStatus,
}

/// One nutrient on one day, before rounding: what the panel shows and a summary adds up.
#[derive(Debug)]
pub(in crate::services::nutrition) struct NutrientDay {
    pub(super) amount: Option<f64>,
    pub(super) recommended: f64,
    /// In the order of the reference's Upper Limits.
    pub(super) limits: Vec<LimitDay>,
}

impl NutrientDay {
    /// The percentage of the Recommended Intake; `None` when the amount is unknown.
    pub(in crate::services::nutrition) fn percent_of_recommended(&self) -> Option<f64> {
        self.amount.map(|amount| amount / self.recommended * 100.0)
    }

    /// Whether the day is above any of the nutrient's Upper Limits (the Day view's caution, or
    /// a day above within a cut down); `None` for a nutrient without one.
    pub(in crate::services::nutrition) fn above_upper_limit(&self) -> Option<bool> {
        (!self.limits.is_empty()).then(|| {
            self.limits
                .iter()
                .any(|limit| limit.amount.is_some_and(|amount| amount > limit.limit))
        })
    }

    /// Whether the trailing 7-day average is above any of its Upper Limits; `None` for a
    /// nutrient without one.
    pub(in crate::services::nutrition) fn cut_down(&self) -> Option<bool> {
        (!self.limits.is_empty()).then(|| {
            self.limits
                .iter()
                .any(|limit| limit.status == LimitStatus::CutDown)
        })
    }
}

/// One day against the references, before rounding.
#[derive(Debug)]
pub(in crate::services::nutrition) struct DayAssessment {
    /// The day has at least one Meal Log.
    pub(in crate::services::nutrition) has_meal_logs: bool,
    /// In the order of [`REFERENCES`].
    pub(in crate::services::nutrition) nutrients: Vec<NutrientDay>,
}
