//! Nutrition Summary: every Trends series over a range of Copenhagen days, by day, ISO week
//! (Monday to Sunday) or calendar month.
//!
//! - **Range**: `from` through `to`, both included, 1 to [`MAX_SUMMARY_DAYS`] days, ending on or
//!   before today. Periods are cut at the range: the first and last week or month cover only
//!   their days inside it and say `partial`, so a 30-day range always sums exactly 30 days.
//! - **Logged days**: a day counts as logged when its Meal Logs have known energy, the rule the
//!   Estimated Expenditure uses. Other days are missing: their intake is left out of the means,
//!   never counted as zero, and a period without a logged day has null intake.
//! - **Intake** (energy, protein and energy per macronutrient at 4/4/9/7 kcal per gram) is the
//!   mean per logged day. A logged day whose Portions carry no value for a macronutrient adds
//!   nothing to it, as on the Nutrition Day; the mean is null when no logged day has a value.
//! - **NOVA 1 share**: Class 1 and classified energy are summed over the period first, then
//!   divided (`NovaEnergy`); daily shares are never averaged.
//! - **Targets**: the Calorie Target, Estimated Expenditure and protein target are the mean of
//!   each day's value over the period's days that have one, from the target engine. Reading a
//!   summary reads its days, so, as for the Nutrition Day, it fixes the weekly target record of
//!   every week up to this one that has none yet (ADR 0014); a backfill afterwards never moves a
//!   Calorie Target the summary has shown.
//! - **Weight**: Trend Weight on the period's last day; scale weight is the mean of its
//!   weigh-ins. The weekly change is the Trend Weight change over the period in kg per week, from
//!   the day before it to its last day, or over the 7 days ending on its last day when the period
//!   is shorter than a week; null without a Trend Weight at both ends. The goal rate is the mean
//!   of each day's Nutrition Goal change in kg per week, negative to lose.
//! - **Micronutrients**: each period's and the range's coverage of the micronutrient panel
//!   (`coverage`), or, without the Profile's sex or birth date or below 18 years on the range's
//!   last day, why there is none; the rest of the summary does not depend on it.
//!
//! One read of the Meal Logs serves everything: the key nutrients and the panel's values, from
//! the 6 days before `from` that the first trailing 7-day averages need through `to`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use super::coverage::{MicronutrientCoverage, PeriodCoverage, Tally};
use super::goals::{self, GoalHistory};
use super::intake::Intake;
use super::meal_logs::{self, MealLogError, Selection, StoredPortion, Values};
use super::micronutrients::{self, Diary, Person};
use super::nova::{NovaEnergy, NovaShare};
use super::parameters;
use super::supplements::StoredDose;
use super::targets::{
    self, KCAL_PER_G_CARBS, KCAL_PER_G_FAT, KCAL_PER_G_PROTEIN, Missing, Plan, TrendWeight,
    round_to,
};
use crate::services::training::calendar::{self, CalendarDay};

/// The longest range one summary covers, in calendar days.
pub const MAX_SUMMARY_DAYS: i64 = 365;
const KCAL_PER_G_ALCOHOL: f64 = 7.0;
/// The shortest span a weekly change is measured over, in days.
const WEEK_DAYS: usize = 7;

/// How days are grouped into periods.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Aggregation {
    Day,
    /// ISO weeks, Monday to Sunday.
    Week,
    /// Calendar months.
    Month,
}

impl Aggregation {
    /// The calendar bounds of the period containing `day`.
    fn bounds(self, day: CalendarDay) -> (CalendarDay, CalendarDay) {
        match self {
            Self::Day => Some((day, day)),
            Self::Week => day.iso_week_bounds(),
            Self::Month => day.month_bounds(),
        }
        .unwrap_or((day, day))
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SummaryQuery {
    pub from: String,
    pub to: String,
    pub aggregation: Aggregation,
}

#[derive(Debug, Serialize)]
pub struct NutritionSummary {
    pub from: CalendarDay,
    pub to: CalendarDay,
    pub aggregation: Aggregation,
    pub time_zone: &'static str,
    /// Days with Meal Logs of known energy in the range.
    pub logged_days: usize,
    /// Days left out of intake means.
    pub missing_days: usize,
    /// Micronutrient coverage over the whole range, or why there is none.
    pub micronutrients: MicronutrientCoverage,
    /// In date order.
    pub periods: Vec<Period>,
}

/// One day, week or month of the range.
#[derive(Debug, Serialize)]
pub struct Period {
    /// The first and last day the period covers inside the range.
    pub start: CalendarDay,
    pub end: CalendarDay,
    pub days: usize,
    /// The calendar week or month reaches past the range, so the period covers only part of it.
    pub partial: bool,
    pub logged_days: usize,
    pub missing_days: usize,
    pub energy: EnergySummary,
    pub protein: ProteinSummary,
    /// Mean energy per logged day from each macronutrient.
    pub macro_energy: MacroEnergy,
    pub nova: NovaShare,
    pub weight: WeightSummary,
    pub weight_change: WeightChange,
    /// Null when the range has no micronutrient coverage.
    pub micronutrients: Option<PeriodCoverage>,
}

/// kcal per day; null when no day has the value.
#[derive(Debug, Serialize)]
pub struct EnergySummary {
    /// Mean per logged day.
    pub eaten_kcal: Option<f64>,
    pub calorie_target_kcal: Option<f64>,
    pub estimated_expenditure_kcal: Option<f64>,
}

/// Grams per day; null when no day has the value.
#[derive(Debug, Serialize)]
pub struct ProteinSummary {
    /// Mean per logged day.
    pub eaten_g: Option<f64>,
    pub target_g: Option<f64>,
}

/// kcal per logged day at 4/4/9/7 kcal per gram.
#[derive(Debug, Serialize)]
pub struct MacroEnergy {
    pub protein_kcal: Option<f64>,
    pub carbohydrate_kcal: Option<f64>,
    pub fat_kcal: Option<f64>,
    pub alcohol_kcal: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct WeightSummary {
    /// On the period's last day.
    pub trend_weight_kg: Option<f64>,
    /// The mean of the period's weigh-ins.
    pub scale_weight_kg: Option<f64>,
    pub weigh_ins: usize,
}

/// kg per week, negative for a loss.
#[derive(Debug, Serialize)]
pub struct WeightChange {
    /// The Trend Weight's change.
    pub kg_per_week: Option<f64>,
    /// The Nutrition Goal's change.
    pub goal_kg_per_week: f64,
}

/// A mean over the values given.
#[derive(Clone, Copy, Debug, Default)]
struct Mean {
    sum: f64,
    count: usize,
}

impl Mean {
    fn add(&mut self, value: Option<f64>) {
        if let Some(value) = value {
            self.sum += value;
            self.count += 1;
        }
    }

    fn get(self) -> Option<f64> {
        (self.count > 0).then(|| self.sum / self.count as f64)
    }
}

/// A sum of known values; `None` until one is added.
#[derive(Clone, Copy, Debug, Default)]
struct Known(Option<f64>);

impl Known {
    fn add(&mut self, value: Option<f64>) {
        if let Some(value) = value {
            *self.0.get_or_insert(0.0) += value;
        }
    }

    /// Per logged day, scaled by `factor` and rounded to `places`.
    fn per_day(self, logged_days: usize, factor: f64, places: i32) -> Option<f64> {
        let sum = self.0.filter(|_| logged_days > 0)?;
        Some(round_to(sum * factor / logged_days as f64, places))
    }
}

/// One day's Portions and Supplement Doses.
#[derive(Default)]
struct Eaten<'a> {
    portions: Vec<&'a StoredPortion>,
    doses: Vec<&'a StoredDose>,
}

impl Eaten<'_> {
    fn total(&self, key: &str) -> Option<f64> {
        Intake::of(key, &self.portions, &self.doses).total()
    }
}

/// A period's days added up.
#[derive(Default)]
struct Sums {
    logged_days: usize,
    energy: Known,
    protein: Known,
    carbs: Known,
    fat: Known,
    alcohol: Known,
    nova: NovaEnergy,
    calorie_target: Mean,
    expenditure: Mean,
    protein_target: Mean,
    scale: Mean,
    goal: Mean,
}

impl Sums {
    /// A day's intake: its NOVA energy always, its macronutrients only when its energy is known.
    /// Returns whether the day counts as logged.
    fn add_eaten(&mut self, eaten: &Eaten<'_>) -> bool {
        self.nova = self.nova + NovaEnergy::of(&eaten.portions);
        let Some(kcal) = eaten.total(parameters::ENERGY_KCAL) else {
            return false;
        };
        self.logged_days += 1;
        self.energy.add(Some(kcal));
        self.protein.add(eaten.total(parameters::PROTEIN));
        self.carbs
            .add(eaten.total(parameters::AVAILABLE_CARBOHYDRATE));
        self.fat.add(eaten.total(parameters::FAT));
        self.alcohol.add(eaten.total(parameters::ALCOHOL));
        true
    }
}

/// What micronutrient coverage is judged from, when the panel is available.
struct CoverageInputs {
    person: Result<Person, Vec<Missing>>,
    diary: Diary,
}

/// Everything read for the range, by day.
struct Inputs<'a> {
    aggregation: Aggregation,
    eaten: BTreeMap<CalendarDay, Eaten<'a>>,
    weigh_ins: BTreeMap<CalendarDay, f64>,
    trend: TrendWeight,
    goals: GoalHistory,
    coverage: Option<CoverageInputs>,
}

impl Inputs<'_> {
    /// One period from its days, which share calendar bounds, with its micronutrient tally.
    fn period(&self, days: &[(CalendarDay, Plan)]) -> Option<(Period, Option<Tally>)> {
        let (start, end) = (days.first()?.0, days.last()?.0);
        let (calendar_start, calendar_end) = self.aggregation.bounds(start);
        let mut sums = Sums::default();
        let mut tally = self.coverage.as_ref().map(|_| Tally::default());
        for (day, plan) in days {
            let logged = self
                .eaten
                .get(day)
                .is_some_and(|eaten| sums.add_eaten(eaten));
            if let (Some(tally), Some(coverage)) = (tally.as_mut(), &self.coverage)
                && let Ok(assessment) = coverage.diary.assess(&coverage.person, *day)
            {
                tally.add(&assessment, logged);
            }
            if let Plan::Available(plan) = plan {
                sums.calorie_target.add(Some(plan.calorie_target_kcal));
                sums.expenditure.add(Some(plan.week.expenditure_kcal));
                sums.protein_target.add(Some(plan.protein_target_g));
            }
            sums.scale.add(self.weigh_ins.get(day).copied());
            sums.goal
                .add(Some(self.goals.on(*day).goal.weekly_change_kg()));
        }

        let logged = sums.logged_days;
        let rounded = |mean: Mean, places| mean.get().map(|value| round_to(value, places));
        let period = Period {
            start,
            end,
            days: days.len(),
            partial: calendar_start < start || calendar_end > end,
            logged_days: logged,
            missing_days: days.len() - logged,
            energy: EnergySummary {
                eaten_kcal: sums.energy.per_day(logged, 1.0, 0),
                calorie_target_kcal: rounded(sums.calorie_target, 0),
                estimated_expenditure_kcal: rounded(sums.expenditure, 0),
            },
            protein: ProteinSummary {
                eaten_g: sums.protein.per_day(logged, 1.0, 1),
                target_g: rounded(sums.protein_target, 1),
            },
            macro_energy: MacroEnergy {
                protein_kcal: sums.protein.per_day(logged, KCAL_PER_G_PROTEIN, 0),
                carbohydrate_kcal: sums.carbs.per_day(logged, KCAL_PER_G_CARBS, 0),
                fat_kcal: sums.fat.per_day(logged, KCAL_PER_G_FAT, 0),
                alcohol_kcal: sums.alcohol.per_day(logged, KCAL_PER_G_ALCOHOL, 0),
            },
            nova: sums.nova.summary(),
            weight: WeightSummary {
                trend_weight_kg: self.trend.on(end).map(|kg| round_to(kg, 2)),
                scale_weight_kg: rounded(sums.scale, 2),
                weigh_ins: sums.scale.count,
            },
            weight_change: WeightChange {
                kg_per_week: self.weekly_change(end, days.len()),
                goal_kg_per_week: rounded(sums.goal, 2).unwrap_or_default(),
            },
            micronutrients: tally.as_ref().map(Tally::period),
        };
        Some((period, tally))
    }

    /// The Trend Weight change in kg per week over the `days` ending on `end`, or over the week
    /// ending on it when `days` is shorter.
    fn weekly_change(&self, end: CalendarDay, days: usize) -> Option<f64> {
        let span = days.max(WEEK_DAYS);
        let before = end.checked_sub_days(u64::try_from(span).ok()?)?;
        let change = self.trend.on(end)? - self.trend.on(before)?;
        Some(round_to(change * WEEK_DAYS as f64 / span as f64, 2))
    }
}

/// Parses the range: 1 to [`MAX_SUMMARY_DAYS`] days ending on or before `today`.
fn range(
    query: &SummaryQuery,
    today: CalendarDay,
) -> Result<(CalendarDay, CalendarDay), MealLogError> {
    let from = meal_logs::parse_day(&query.from, "from")?;
    let to = meal_logs::parse_day(&query.to, "to")?;
    if !(0..MAX_SUMMARY_DAYS).contains(&from.days_until(to)) {
        return Err(MealLogError::Validation(format!(
            "from and to must cover 1 to {MAX_SUMMARY_DAYS} calendar days, with from <= to"
        )));
    }
    if to > today {
        return Err(MealLogError::Validation(format!(
            "to must not be after today ({today}) in {}",
            calendar::TIME_ZONE_NAME
        )));
    }
    Ok((from, to))
}

/// The summary of `query`'s range by its aggregation. Every input is read once for the whole
/// range; reading may fix weekly target records (`targets::plan`), with `today` deciding which
/// weeks have started.
pub async fn nutrition_summary(
    db: &SqlitePool,
    query: &SummaryQuery,
    today: CalendarDay,
) -> Result<NutritionSummary, MealLogError> {
    let (from, to) = range(query, today)?;
    let plans = targets::plan(db, from, to, today).await?;
    let person = Person::read(db).await?;
    let unavailable = micronutrients::unavailable_on(&person, to);
    let (first_day, keys) = if unavailable.is_some() {
        (from, meal_logs::KEY_NUTRIENTS.to_vec())
    } else {
        let mut keys = micronutrients::value_keys();
        keys.extend(meal_logs::KEY_NUTRIENTS);
        (micronutrients::trailing_start(from), keys)
    };
    let logs = meal_logs::load(db, &Selection::Days(first_day, to), Values::Only(&keys)).await?;
    let weigh_ins = targets::weigh_ins(db, to).await?;
    let goals = goals::history(db).await?;

    let mut eaten: BTreeMap<CalendarDay, Eaten<'_>> = BTreeMap::new();
    for log in &logs {
        let Some(day) = CalendarDay::parse(&log.day).filter(|day| *day >= from) else {
            continue;
        };
        let entry = eaten.entry(day).or_default();
        entry.portions.extend(&log.portions);
        entry.doses.extend(&log.doses);
    }
    let inputs = Inputs {
        aggregation: query.aggregation,
        eaten,
        weigh_ins: weigh_ins
            .iter()
            .filter(|(day, _)| *day >= from)
            .copied()
            .collect(),
        trend: TrendWeight::of(&weigh_ins),
        goals,
        coverage: unavailable.is_none().then(|| CoverageInputs {
            person,
            diary: Diary::of(&logs),
        }),
    };
    let mut range_tally = Tally::default();
    let periods: Vec<Period> = plans
        .chunk_by(|(left, _), (right, _)| {
            query.aggregation.bounds(*left) == query.aggregation.bounds(*right)
        })
        .filter_map(|days| inputs.period(days))
        .map(|(period, tally)| {
            if let Some(tally) = &tally {
                range_tally += tally;
            }
            period
        })
        .collect();
    let micronutrients = match unavailable {
        Some(unavailable) => MicronutrientCoverage::Unavailable(unavailable),
        None => MicronutrientCoverage::Available(Box::new(range_tally.range())),
    };
    let logged_days = periods.iter().map(|period| period.logged_days).sum();
    let missing_days = periods.iter().map(|period| period.missing_days).sum();
    Ok(NutritionSummary {
        from,
        to,
        aggregation: query.aggregation,
        time_zone: calendar::TIME_ZONE_NAME,
        logged_days,
        missing_days,
        micronutrients,
        periods,
    })
}
