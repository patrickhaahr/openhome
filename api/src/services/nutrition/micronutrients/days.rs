use super::references::{Measure, Reference, Share};
use super::{DHA, EPA, LimitStatus, RETINOL};
use crate::services::nutrition::intake::Intake;
use crate::services::nutrition::supplements::StoredDose;
use crate::services::nutrition::targets::{Missing, Sex};
use crate::services::training::calendar::CalendarDay;
use sqlx::SqlitePool;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug)]
pub struct Person {
    pub(super) sex: Sex,
    pub(super) birth_date: CalendarDay,
}

impl Person {
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

#[derive(Debug, Default)]
pub(super) struct DayEaten {
    pub(super) food: HashMap<String, (f64, usize)>,
    pub(super) portions: usize,
    pub(super) doses: Vec<StoredDose>,
}

impl DayEaten {
    fn intake(&self, key: &str) -> Intake {
        let doses: Vec<&StoredDose> = self.doses.iter().collect();
        let food = match self.food.get(key) {
            Some(&(amount, known)) => Intake::from_food(Some(amount), self.portions - known),
            None => Intake::from_food(None, self.portions),
        };
        food + Intake::of(key, &[], &doses)
    }

    fn supplement_form(&self, keys: &[&str], form: &str) -> f64 {
        keys.iter()
            .filter_map(|key| self.intake(key).supplement_form(form))
            .sum()
    }
}

#[derive(Clone, Debug)]
pub(super) struct DayIntake {
    pub(super) amount: Option<f64>,
    pub(super) food: Option<f64>,
    pub(super) supplements: Option<f64>,
    pub(super) unknown_portions: usize,
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

#[derive(Debug)]
pub(super) struct LimitDay {
    pub(super) limit: f64,
    pub(super) amount: Option<f64>,
    pub(super) average: Option<f64>,
    pub(super) averaged_days: usize,
    pub(super) status: LimitStatus,
}

#[derive(Debug)]
pub(in crate::services::nutrition) struct NutrientDay {
    pub(super) amount: Option<f64>,
    pub(super) recommended: f64,
    pub(super) limits: Vec<LimitDay>,
}

impl NutrientDay {
    pub(in crate::services::nutrition) fn percent_of_recommended(&self) -> Option<f64> {
        self.amount.map(|amount| amount / self.recommended * 100.0)
    }

    pub(in crate::services::nutrition) fn above_upper_limit(&self) -> Option<bool> {
        (!self.limits.is_empty()).then(|| {
            self.limits
                .iter()
                .any(|limit| limit.amount.is_some_and(|amount| amount > limit.limit))
        })
    }

    pub(in crate::services::nutrition) fn cut_down(&self) -> Option<bool> {
        (!self.limits.is_empty()).then(|| {
            self.limits
                .iter()
                .any(|limit| limit.status == LimitStatus::CutDown)
        })
    }
}

#[derive(Debug)]
pub(in crate::services::nutrition) struct DayAssessment {
    pub(in crate::services::nutrition) has_meal_logs: bool,
    pub(in crate::services::nutrition) nutrients: Vec<NutrientDay>,
}
