use super::references::{AgeBand, REFERENCE_ENERGY_MJ, REFERENCES};
use super::{
    DayAssessment, DayEaten, DayIntake, LimitDay, LimitStatus, MIN_CUT_DOWN_DAYS, Micronutrients,
    NutrientDay, NutrientStatus, Panel, Person, SOURCES, TRAILING_DAYS, TrailingWindow,
    Unavailable, value_keys,
};
use crate::services::nutrition::meal_logs::MealLogError;
use crate::services::nutrition::supplements;
use crate::services::nutrition::targets::{Missing, Sex};
use crate::services::training::calendar::CalendarDay;
use sqlx::SqlitePool;
use std::collections::BTreeMap;

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
    pub(in crate::services::nutrition) fn assess(
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
                        let habitual = averaged_days >= MIN_CUT_DOWN_DAYS
                            && average.is_some_and(|average| average > limit.value);
                        let status = if habitual {
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
pub(in crate::services::nutrition) fn trailing_start(day: CalendarDay) -> CalendarDay {
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
pub(in crate::services::nutrition) fn unavailable_on(
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
