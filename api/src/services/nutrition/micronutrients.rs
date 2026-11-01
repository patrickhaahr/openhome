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
//! days, so a range is loaded once and each day is evaluated from it: as the Nutrition Day's
//! panel ([`Diary::evaluate`]), or unrounded for the Nutrition Summary to add up
//! ([`Diary::assess`]).

pub mod references;

use std::collections::{BTreeMap, HashMap};

use serde::Serialize;
use sqlx::SqlitePool;

use self::references::{
    AgeBand, Group, Measure, REFERENCE_ENERGY_MJ, REFERENCES, Reference, Share,
};
use super::intake::Intake;
use super::meal_logs::{MealLogError, round};
use super::supplements::{self, StoredDose};
use super::targets::{Missing, Sex};
use crate::services::training::calendar::CalendarDay;

/// Days the trailing average spans, ending with the day evaluated.
pub const TRAILING_DAYS: u64 = 7;
const EPA: &str = "epa";
const DHA: &str = "dha";
/// Preformed vitamin A in food.
const RETINOL: &str = "retinol";

/// Every Portion value the panel reads: its parameters, EPA and DHA, and food retinol.
/// Supplement Doses always carry every amount, chemical forms included.
pub(super) fn value_keys() -> Vec<&'static str> {
    let mut keys: Vec<&'static str> = REFERENCES
        .iter()
        .filter_map(|reference| match reference.measure {
            Measure::Parameter(key) => Some(key),
            Measure::EpaDha => None,
        })
        .chain([EPA, DHA, RETINOL])
        .collect();
    keys.sort_unstable();
    keys.dedup();
    keys
}

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

/// One logged day's Portions summed per parameter in SQL, with its Supplement Doses: what the
/// panel reads, without loading every Portion's values.
#[derive(Debug, Default)]
struct DayEaten {
    /// Per parameter key: the amount from the Portions that have a value, and how many do.
    food: HashMap<String, (f64, usize)>,
    portions: usize,
    doses: Vec<StoredDose>,
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
struct DayIntake {
    amount: Option<f64>,
    food: Option<f64>,
    supplements: Option<f64>,
    unknown_portions: usize,
    /// The share each of the reference's Upper Limits applies to, in its order.
    limited: Vec<Option<f64>>,
}

impl DayIntake {
    fn of(reference: &Reference, eaten: &DayEaten) -> Self {
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

impl NutrientStatus {
    fn new(reference: &'static Reference, day: NutrientDay, intake: Option<&DayIntake>) -> Self {
        let upper_limits: Vec<UpperLimitStatus> = reference
            .upper_limits
            .iter()
            .zip(day.limits)
            .map(|(limit, limit_day)| UpperLimitStatus {
                applies_to: limit.share,
                limit: limit.value,
                amount: limit_day.amount.map(round),
                seven_day_average: limit_day.average.map(round),
                averaged_days: limit_day.averaged_days,
                status: limit_day.status,
                source: limit.source,
            })
            .collect();
        Self {
            key: reference.key,
            name: reference.name,
            group: reference.group,
            unit: reference.unit,
            amount: day.amount.map(round),
            food: intake.and_then(|intake| intake.food).map(round),
            supplements: intake.and_then(|intake| intake.supplements).map(round),
            unknown_portions: intake.map_or(0, |intake| intake.unknown_portions),
            recommended_intake: round(day.recommended),
            adequate_intake: reference.adequate,
            percent_of_recommended: day
                .amount
                .map(|amount| (amount / day.recommended * 1000.0).round() / 10.0),
            source: reference.source,
            upper_limit_status: upper_limits.iter().map(|limit| limit.status).max(),
            upper_limits,
        }
    }
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

/// Why there is no panel.
#[derive(Debug, Serialize)]
pub struct Unavailable {
    /// Profile values to add; empty when the age is outside the adult references.
    pub missing: Vec<Missing>,
    pub reason: String,
}

/// A day's micronutrient panel, or why there is none.
#[derive(Debug, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Micronutrients {
    Available(Box<Panel>),
    Unavailable(Unavailable),
}

/// One Upper Limit on one day, before rounding.
#[derive(Debug)]
struct LimitDay {
    limit: f64,
    /// The day's amount of the share the limit applies to.
    amount: Option<f64>,
    average: Option<f64>,
    averaged_days: usize,
    status: LimitStatus,
}

/// One nutrient on one day, before rounding: what the panel shows and a summary adds up.
#[derive(Debug)]
pub(super) struct NutrientDay {
    amount: Option<f64>,
    recommended: f64,
    /// In the order of the reference's Upper Limits.
    limits: Vec<LimitDay>,
}

impl NutrientDay {
    /// The percentage of the Recommended Intake; `None` when the amount is unknown.
    pub(super) fn percent_of_recommended(&self) -> Option<f64> {
        self.amount.map(|amount| amount / self.recommended * 100.0)
    }

    /// Whether the day is above any of the nutrient's Upper Limits (the Day view's caution, or
    /// a day above within a cut down); `None` for a nutrient without one.
    pub(super) fn above_upper_limit(&self) -> Option<bool> {
        (!self.limits.is_empty()).then(|| {
            self.limits
                .iter()
                .any(|limit| limit.amount.is_some_and(|amount| amount > limit.limit))
        })
    }

    /// Whether the trailing 7-day average is above any of its Upper Limits; `None` for a
    /// nutrient without one.
    pub(super) fn cut_down(&self) -> Option<bool> {
        (!self.limits.is_empty()).then(|| {
            self.limits
                .iter()
                .any(|limit| limit.status == LimitStatus::CutDown)
        })
    }
}

/// One day against the references, before rounding.
#[derive(Debug)]
pub(super) struct DayAssessment {
    /// The day has at least one Meal Log.
    pub(super) has_meal_logs: bool,
    /// In the order of [`REFERENCES`].
    pub(super) nutrients: Vec<NutrientDay>,
}

/// Every logged day's intake of the panel's nutrients.
#[derive(Debug, Default)]
pub struct Diary {
    days: BTreeMap<CalendarDay, Vec<DayIntake>>,
}

impl Diary {
    /// The Meal Logs needed to evaluate every day from `from` through `to`, including the
    /// trailing window before `from`. One query sums each day's Portions per parameter, so a
    /// year reads one row per day rather than every Portion value.
    pub async fn load(
        db: &SqlitePool,
        from: CalendarDay,
        to: CalendarDay,
    ) -> Result<Self, MealLogError> {
        let (start, end) = (trailing_start(from).to_string(), to.to_string());
        let keys = serde_json::Value::from(value_keys()).to_string();
        let rows = sqlx::query!(
            r#"WITH k (key) AS MATERIALIZED (SELECT value FROM json_each(?3)),
            sums AS (
                SELECT m.day, k.key, SUM(v.value * p.grams / 100.0) AS food, COUNT(*) AS known
                FROM meal_logs m CROSS JOIN meal_log_portions p ON p.meal_log_id = m.id
                CROSS JOIN k CROSS JOIN meal_log_portion_values v
                    ON v.portion_id = p.id AND v.key = k.key
                WHERE m.day >= ?1 AND m.day <= ?2
                GROUP BY m.day, k.key
            ),
            food AS (
                SELECT day, json_group_object(key, json_array(food, known)) AS food
                FROM sums GROUP BY day
            ),
            days AS (
                SELECT m.day, json_group_array(m.id) AS logs,
                    SUM((SELECT COUNT(*) FROM meal_log_portions p WHERE p.meal_log_id = m.id))
                        AS portions
                FROM meal_logs m WHERE m.day >= ?1 AND m.day <= ?2 GROUP BY m.day
            )
            SELECT d.day AS "day!", d.logs AS "logs!: String", d.portions AS "portions!: i64",
                COALESCE(f.food, '{}') AS "food!: String"
            FROM days d LEFT JOIN food f ON f.day = d.day ORDER BY d.day"#,
            start,
            end,
            keys
        )
        .fetch_all(db)
        .await?;
        let mut doses = supplements::log_doses(db, None, Some(&start), Some(&end)).await?;
        let mut days = BTreeMap::new();
        for row in rows {
            let Some(day) = CalendarDay::parse(&row.day) else {
                continue;
            };
            let logs: Vec<String> = serde_json::from_str(&row.logs).map_err(decode_error)?;
            let eaten = DayEaten {
                food: serde_json::from_str(&row.food).map_err(decode_error)?,
                portions: usize::try_from(row.portions).unwrap_or_default(),
                doses: logs
                    .iter()
                    .filter_map(|log| doses.remove(log))
                    .flatten()
                    .collect(),
            };
            let intakes = REFERENCES
                .iter()
                .map(|reference| DayIntake::of(reference, &eaten))
                .collect();
            days.insert(day, intakes);
        }
        Ok(Self { days })
    }

    /// The panel for `day`, which must lie in the range the diary was loaded for.
    #[must_use]
    pub fn evaluate(
        &self,
        person: &Result<Person, Vec<Missing>>,
        day: CalendarDay,
    ) -> Micronutrients {
        let (person, age, band) = match references_on(person, day) {
            Ok(references) => references,
            Err(unavailable) => return Micronutrients::Unavailable(unavailable),
        };
        let (from, to) = window(day);
        let intakes = self.days.get(&day);
        let nutrients = REFERENCES
            .iter()
            .zip(self.nutrients(person.sex, band, day))
            .enumerate()
            .map(|(index, (reference, nutrient))| {
                NutrientStatus::new(reference, nutrient, intakes.map(|intakes| &intakes[index]))
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

    /// `day` against the references, unrounded, exactly as [`Self::evaluate`] judges it; or why
    /// there is no panel that day. `day` must lie in the range the diary was loaded for.
    pub(super) fn assess(
        &self,
        person: &Result<Person, Vec<Missing>>,
        day: CalendarDay,
    ) -> Result<DayAssessment, Unavailable> {
        let (person, _, band) = references_on(person, day)?;
        Ok(DayAssessment {
            has_meal_logs: self.days.contains_key(&day),
            nutrients: self.nutrients(person.sex, band, day),
        })
    }

    /// Each nutrient of `day` in the order of [`REFERENCES`].
    fn nutrients(&self, sex: Sex, band: AgeBand, day: CalendarDay) -> Vec<NutrientDay> {
        let intakes = self.days.get(&day);
        REFERENCES
            .iter()
            .enumerate()
            .map(|(index, reference)| {
                let intake = intakes.map(|intakes| &intakes[index]);
                let limits = reference
                    .upper_limits
                    .iter()
                    .enumerate()
                    .map(|(limit_index, limit)| {
                        let amount = intake.and_then(|intake| intake.limited[limit_index]);
                        let (average, averaged_days) =
                            self.trailing_average(index, limit_index, day);
                        let status = if average.is_some_and(|average| average > limit.value) {
                            LimitStatus::CutDown
                        } else if amount.is_some_and(|amount| amount > limit.value) {
                            LimitStatus::Caution
                        } else {
                            LimitStatus::Clear
                        };
                        LimitDay {
                            limit: limit.value,
                            amount,
                            average,
                            averaged_days,
                            status,
                        }
                    })
                    .collect();
                NutrientDay {
                    amount: intake.and_then(|intake| intake.amount),
                    recommended: reference.recommended.daily(sex, band),
                    limits,
                }
            })
            .collect()
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

/// SQLite returned JSON the query itself built in another shape.
fn decode_error(error: serde_json::Error) -> MealLogError {
    MealLogError::Database(sqlx::Error::Decode(Box::new(error)))
}

/// The trailing window ending with `day`.
fn window(day: CalendarDay) -> (CalendarDay, CalendarDay) {
    (day.checked_sub_days(TRAILING_DAYS - 1).unwrap_or(day), day)
}

/// The first day whose Meal Logs evaluating `day` reads.
pub(super) fn trailing_start(day: CalendarDay) -> CalendarDay {
    window(day).0
}

/// The person, their age and age band on `day`, or why the references do not cover them.
fn references_on(
    person: &Result<Person, Vec<Missing>>,
    day: CalendarDay,
) -> Result<(Person, u32, AgeBand), Unavailable> {
    let person = match person {
        Ok(person) => *person,
        Err(missing) => {
            return Err(Unavailable {
                reason: missing_reason(missing),
                missing: missing.clone(),
            });
        }
    };
    let Some(age) = day.years_since(person.birth_date) else {
        return Err(Unavailable {
            missing: Vec::new(),
            reason: format!(
                "The micronutrient panel needs a day on or after the Profile's birth date, not {day}"
            ),
        });
    };
    let Some(band) = AgeBand::of(age) else {
        return Err(Unavailable {
            missing: Vec::new(),
            reason: format!(
                "The micronutrient panel covers adults from 18; on {day} the Profile's birth \
                 date gives {age}"
            ),
        });
    };
    Ok((person, age, band))
}

/// Why there is no panel on `day`, if there is none. A panel on the last day of a range means
/// every earlier day the person was an adult has one too, since age only grows.
pub(super) fn unavailable_on(
    person: &Result<Person, Vec<Missing>>,
    day: CalendarDay,
) -> Option<Unavailable> {
    references_on(person, day).err()
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
