//! Micronutrient evaluation (`CONTEXT.md`, Recommended Intake, Upper Limit): each vitamin and
//! mineral the day supplied, as a percentage of its NNR 2023 Recommended Intake for the Profile's
//! sex and age band, and checked against its EFSA Upper Limits.
//!
//! - The age band comes from the Profile's birth date and the day evaluated, so a birthday moves
//!   later days to the next band. Without the Profile's sex or birth date, or below 18 years, the
//!   panel is unavailable with the reason; nothing is guessed.
//! - Reaching or passing a Recommended Intake is good and is never flagged.
//! - An Upper Limit is checked against the share of intake it applies to ([`Share`]): food and
//!   supplements, or only the supplement share or one chemical form. A day above it is a
//!   **caution**. A **cut down** applies when the trailing 7-day average is above it: the mean
//!   over the days with at least one Meal Log among that day and the 6 before it. Days without
//!   Meal Logs are left out rather than counted as zero, as they are for the Estimated
//!   Expenditure, so an untracked day never dilutes a habitual excess. A logged day without
//!   Supplement Doses of the nutrient adds zero to a supplement-only share; a logged day whose
//!   food value is unknown is left out of a share that includes food, because unknown is never
//!   zero. Cut down outranks caution. Nutrients without an Upper Limit carry no status at all.
//! - EPA+DHA adds food and supplements against EFSA's adequate intake of 250 mg. Compounds
//!   without a database parameter (creatine) have no reference; the Nutrition Day lists them as
//!   amounts taken.
//!
//! [`Diary`] holds every logged day's intake of the panel's nutrients and evaluates any of its
//! days, so a range is loaded once and each day is evaluated from it.

pub mod references;

use std::collections::BTreeMap;

use serde::Serialize;
use sqlx::SqlitePool;

use self::references::{
    AgeBand, Group, Measure, REFERENCE_ENERGY_MJ, REFERENCES, Reference, Share,
};
use super::intake::Intake;
use super::meal_logs::{self, MealLogError, Selection, StoredLog, StoredPortion, round};
use super::supplements::StoredDose;
use super::targets::{Missing, Sex};
use crate::services::training::calendar::CalendarDay;

/// Days the trailing average spans, ending with the day evaluated.
pub const TRAILING_DAYS: u64 = 7;

/// The Profile values the references depend on.
#[derive(Clone, Copy, Debug)]
pub struct Person {
    sex: Sex,
    birth_date: CalendarDay,
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

/// The supplement share of `form` over `keys`, zero when none was taken.
fn supplement_form(
    keys: &[&str],
    form: &str,
    portions: &[&StoredPortion],
    doses: &[&StoredDose],
) -> f64 {
    keys.iter()
        .filter_map(|key| Intake::of(key, portions, doses).supplement_form(form))
        .sum()
}

/// One nutrient's intake on one logged day.
#[derive(Clone, Debug)]
struct DayIntake {
    amount: Option<f64>,
    food: Option<f64>,
    supplements: Option<f64>,
    unknown_portions: usize,
    /// The share each of the reference's Upper Limits applies to, in its order.
    limited: Vec<Option<f64>>,
}

impl DayIntake {
    fn of(reference: &Reference, portions: &[&StoredPortion], doses: &[&StoredDose]) -> Self {
        let (amount, food, supplements, unknown_portions) = match reference.measure {
            Measure::Parameter(key) => {
                let intake = Intake::of(key, portions, doses);
                (
                    intake.total(),
                    intake.food(),
                    intake.supplements(),
                    intake.unknown_portions(),
                )
            }
            Measure::EpaDha => {
                let (epa, dha) = (
                    Intake::of("epa", portions, doses),
                    Intake::of("dha", portions, doses),
                );
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
                    Intake::of("retinol", portions, doses).total(),
                    Intake::of("vitamin_a", portions, doses).supplement_form("retinol"),
                ),
                Share::SupplementalFolate => Some(
                    supplement_form(&["folate"], "folic_acid", portions, doses)
                        + supplement_form(&["folate"], "methylfolate", portions, doses),
                ),
                Share::NicotinicAcid => Some(supplement_form(
                    &["niacin_equivalent", "niacin"],
                    "nicotinic_acid",
                    portions,
                    doses,
                )),
                Share::Nicotinamide => Some(supplement_form(
                    &["niacin_equivalent", "niacin"],
                    "nicotinamide",
                    portions,
                    doses,
                )),
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

/// Where a nutrient stands against an Upper Limit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LimitStatus {
    /// Neither the day nor the trailing average is above the limit.
    #[serde(rename = "none")]
    Clear,
    /// The day is above the limit.
    Caution,
    /// The trailing 7-day average is above the limit.
    CutDown,
}

/// One Upper Limit with the day's share it applies to and the trailing average of that share.
#[derive(Debug, Serialize)]
pub struct UpperLimitStatus {
    pub applies_to: Share,
    pub limit: f64,
    /// The day's amount of the share; null when the day has no Meal Logs or its value is unknown.
    pub amount: Option<f64>,
    /// The mean over the logged days of the trailing window with a known amount.
    pub seven_day_average: Option<f64>,
    pub averaged_days: usize,
    pub status: LimitStatus,
    pub source: &'static str,
}

/// One nutrient of the panel.
#[derive(Debug, Serialize)]
pub struct NutrientStatus {
    pub key: &'static str,
    pub name: &'static str,
    pub group: Group,
    pub unit: &'static str,
    /// Food and supplements together; null when nothing logged that day has a value.
    pub amount: Option<f64>,
    pub food: Option<f64>,
    pub supplements: Option<f64>,
    /// Portions without a value, so the amount is at least what is shown.
    pub unknown_portions: usize,
    pub recommended_intake: f64,
    /// An NNR adequate intake (AI), less certain than a recommended intake (RI).
    pub adequate_intake: bool,
    pub percent_of_recommended: Option<f64>,
    pub source: &'static str,
    /// Empty for a nutrient without an Upper Limit.
    pub upper_limits: Vec<UpperLimitStatus>,
    /// The most serious status of its Upper Limits; null without any.
    pub upper_limit_status: Option<LimitStatus>,
}

/// The trailing window the averages span.
#[derive(Debug, Serialize)]
pub struct TrailingWindow {
    pub from: CalendarDay,
    pub to: CalendarDay,
    /// Days with at least one Meal Log in it.
    pub logged_days: usize,
}

/// A source the panel's references come from.
#[derive(Debug, Serialize)]
pub struct Citation {
    pub name: &'static str,
    pub citation: &'static str,
}

/// The sources of [`REFERENCES`], to credit beside the panel.
pub const SOURCES: &[Citation] = &[
    Citation {
        name: "NNR 2023",
        citation: "Nordic Nutrition Recommendations 2023, Nordic Council of Ministers, \
            doi:10.6027/nord2023-003",
    },
    Citation {
        name: "EFSA Upper Limits",
        citation: "EFSA, Overview on Tolerable Upper Intake Levels, Version 11 (August 2025)",
    },
    Citation {
        name: "EFSA 2010",
        citation: "EFSA NDA Panel, Dietary Reference Values for fats, EFSA Journal \
            8(3):1461 (EPA+DHA)",
    },
];

#[derive(Debug, Serialize)]
pub struct Panel {
    pub sex: &'static str,
    /// Whole years on the day evaluated.
    pub age: u32,
    pub age_band: &'static str,
    /// The band's reference energy intake that per-MJ references are converted with.
    pub reference_energy_mj: f64,
    pub trailing_window: TrailingWindow,
    pub nutrients: Vec<NutrientStatus>,
    pub sources: &'static [Citation],
}

/// A day's micronutrient panel, or why there is none.
#[derive(Debug, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Micronutrients {
    Available(Box<Panel>),
    Unavailable {
        /// Profile values to add; empty when the age is outside the adult references.
        missing: Vec<Missing>,
        reason: String,
    },
}

/// Every logged day's intake of the panel's nutrients.
#[derive(Debug, Default)]
pub struct Diary {
    days: BTreeMap<CalendarDay, Vec<DayIntake>>,
}

impl Diary {
    /// The logged days of `logs`.
    pub(super) fn of(logs: &[StoredLog]) -> Self {
        let mut by_day: BTreeMap<CalendarDay, (Vec<&StoredPortion>, Vec<&StoredDose>)> =
            BTreeMap::new();
        for log in logs {
            let Some(day) = CalendarDay::parse(&log.day) else {
                continue;
            };
            let (portions, doses) = by_day.entry(day).or_default();
            portions.extend(&log.portions);
            doses.extend(&log.doses);
        }
        let days = by_day
            .into_iter()
            .map(|(day, (portions, doses))| {
                let intakes = REFERENCES
                    .iter()
                    .map(|reference| DayIntake::of(reference, &portions, &doses))
                    .collect();
                (day, intakes)
            })
            .collect();
        Self { days }
    }

    /// The Meal Logs needed to evaluate every day from `from` through `to`, including the
    /// trailing window before `from`.
    pub async fn load(
        db: &SqlitePool,
        from: CalendarDay,
        to: CalendarDay,
    ) -> Result<Self, MealLogError> {
        let start = window(from).0;
        let logs = meal_logs::load(db, &Selection::Days(start, to), true).await?;
        Ok(Self::of(&logs))
    }

    /// The panel for `day`, which must lie in the range the diary was loaded for.
    #[must_use]
    pub fn evaluate(
        &self,
        person: &Result<Person, Vec<Missing>>,
        day: CalendarDay,
    ) -> Micronutrients {
        let person = match person {
            Ok(person) => *person,
            Err(missing) => {
                return Micronutrients::Unavailable {
                    reason: missing_reason(missing),
                    missing: missing.clone(),
                };
            }
        };
        let Some(age) = day.years_since(person.birth_date) else {
            return Micronutrients::Unavailable {
                missing: Vec::new(),
                reason: format!(
                    "The micronutrient panel needs a day on or after the Profile's birth date, not {day}"
                ),
            };
        };
        let Some(band) = AgeBand::of(age) else {
            return Micronutrients::Unavailable {
                missing: Vec::new(),
                reason: format!(
                    "The micronutrient panel covers adults from 18; on {day} the Profile's birth \
                     date gives {age}"
                ),
            };
        };
        let (from, to) = window(day);
        let intakes = self.days.get(&day);
        let nutrients = REFERENCES
            .iter()
            .enumerate()
            .map(|(index, reference)| {
                self.status(
                    index,
                    reference,
                    intakes.map(|intakes| &intakes[index]),
                    person.sex,
                    band,
                    day,
                )
            })
            .collect();
        Micronutrients::Available(Box::new(Panel {
            sex: match person.sex {
                Sex::Female => "female",
                Sex::Male => "male",
            },
            age,
            age_band: band.label(),
            reference_energy_mj: REFERENCE_ENERGY_MJ.get(person.sex, band),
            trailing_window: TrailingWindow {
                from,
                to,
                logged_days: self.days.range(from..=to).count(),
            },
            nutrients,
            sources: SOURCES,
        }))
    }

    fn status(
        &self,
        index: usize,
        reference: &'static Reference,
        intake: Option<&DayIntake>,
        sex: Sex,
        band: AgeBand,
        day: CalendarDay,
    ) -> NutrientStatus {
        let recommended = reference.recommended.daily(sex, band);
        let amount = intake.and_then(|intake| intake.amount);
        let upper_limits: Vec<UpperLimitStatus> = reference
            .upper_limits
            .iter()
            .enumerate()
            .map(|(limit_index, limit)| {
                let day_amount = intake.and_then(|intake| intake.limited[limit_index]);
                let (average, averaged_days) = self.trailing_average(index, limit_index, day);
                let status = if average.is_some_and(|average| average > limit.value) {
                    LimitStatus::CutDown
                } else if day_amount.is_some_and(|amount| amount > limit.value) {
                    LimitStatus::Caution
                } else {
                    LimitStatus::Clear
                };
                UpperLimitStatus {
                    applies_to: limit.share,
                    limit: limit.value,
                    amount: day_amount.map(round),
                    seven_day_average: average.map(round),
                    averaged_days,
                    status,
                    source: limit.source,
                }
            })
            .collect();
        NutrientStatus {
            key: reference.key,
            name: reference.name,
            group: reference.group,
            unit: reference.unit,
            amount: amount.map(round),
            food: intake.and_then(|intake| intake.food).map(round),
            supplements: intake.and_then(|intake| intake.supplements).map(round),
            unknown_portions: intake.map_or(0, |intake| intake.unknown_portions),
            recommended_intake: round(recommended),
            adequate_intake: reference.adequate,
            percent_of_recommended: amount
                .map(|amount| (amount / recommended * 1000.0).round() / 10.0),
            source: reference.source,
            upper_limit_status: upper_limits.iter().map(|limit| limit.status).max(),
            upper_limits,
        }
    }

    /// The mean of one Upper Limit's share over the logged days of `day`'s trailing window that
    /// know it, and how many days that is.
    fn trailing_average(
        &self,
        nutrient: usize,
        limit: usize,
        day: CalendarDay,
    ) -> (Option<f64>, usize) {
        let (from, to) = window(day);
        let known: Vec<f64> = self
            .days
            .range(from..=to)
            .filter_map(|(_, intakes)| intakes[nutrient].limited[limit])
            .collect();
        let average = (!known.is_empty()).then(|| known.iter().sum::<f64>() / known.len() as f64);
        (average, known.len())
    }
}

/// The trailing window ending with `day`.
fn window(day: CalendarDay) -> (CalendarDay, CalendarDay) {
    (day.checked_sub_days(TRAILING_DAYS - 1).unwrap_or(day), day)
}

/// Why the panel is unavailable, naming the Profile values to add.
fn missing_reason(missing: &[Missing]) -> String {
    let names: Vec<&str> = missing
        .iter()
        .filter_map(|missing| match missing {
            Missing::Sex => Some("sex (male or female)"),
            Missing::BirthDate => Some("birth date"),
            Missing::Height | Missing::BodyWeight => None,
        })
        .collect();
    match names.as_slice() {
        [] => "The micronutrient panel is unavailable".to_owned(),
        [only] => format!("The micronutrient panel needs the Profile's {only}"),
        [rest @ .., last] => format!(
            "The micronutrient panel needs the Profile's {} and {last}",
            rest.join(", ")
        ),
    }
}
