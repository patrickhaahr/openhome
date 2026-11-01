//! Nutrition Summary: every Trends series over a range of Copenhagen days, by day, ISO week
//! (Monday to Sunday) or calendar month.
//!
//! - **Range**: `from` through `to`, both included, 1 to [`MAX_SUMMARY_DAYS`] days, ending on or
//!   before today. Periods are cut at the range: the first and last week or month cover only
//!   their days inside it and say `partial`, so a 30-day range always sums exactly 30 days.
//! - **Logged days**: a day counts as logged when it has a Meal Log, even one of Supplement Doses
//!   alone. Other days are missing: their intake is left out of the means, never counted as zero,
//!   and a period without a logged day has null intake.
//! - **Intake** (energy, protein and energy per macronutrient at 4/4/9/7 kcal per gram) is the
//!   mean over the logged days that know each metric (`known_days`); `intake_days` counts
//!   known-energy days. Each metric is independent, so unknown energy never erases known
//!   protein and an unknown value never contributes a zero. Estimated Expenditure still learns
//!   from known-energy days only (`targets`).
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
//! - **Sources**: the Danish Food Composition Database versions the range's Portions copied
//!   their values from, and Open Food Facts when a Product Portion is in the range, to credit
//!   wherever the summary is shown.
//! - **Micronutrients**: each period's and the range's coverage of the micronutrient panel
//!   (`coverage`), or, without the Profile's sex or birth date or below 18 years on the range's
//!   last day, why there is none; the rest of the summary does not depend on it.
//!
//! Every input is read once for the whole range: the Meal Logs with their key nutrients, and for
//! the coverage the panel's day totals (`Diary::load`), from the 6 days before `from` that the
//! first trailing 7-day averages need through `to`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use super::coverage::{MicronutrientCoverage, PeriodCoverage, Tally};
use super::food_database::{self, Source};
use super::goals::{self, GoalHistory};
use super::intake::Intake;
use super::meal_logs::{self, MealLogError, Selection, StoredPortion, Values};
use super::micronutrients::{self, Diary, Person};
use super::nova::{NovaEnergy, NovaShare};
use super::parameters;
use super::products::{LABEL_SOURCE, LabelSource};
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
    /// Days with Meal Logs in the range.
    pub logged_days: usize,
    /// Days left out of intake means.
    pub missing_days: usize,
    /// Micronutrient coverage over the whole range, or why there is none.
    pub micronutrients: MicronutrientCoverage,
    /// The Danish Food Composition Database versions the range's Portions copied their values
    /// from, sorted by version; empty without Portions.
    pub sources: Vec<Source>,
    /// Open Food Facts when a Product Portion is in the range; null otherwise.
    pub label_source: Option<LabelSource>,
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
    /// Logged days whose energy is known: the days the energy mean covers.
    pub intake_days: usize,
    /// Known logged days for each metric; an unknown value is never a day of zero intake.
    pub known_days: KnownDays,
    pub energy: EnergySummary,
    pub protein: ProteinSummary,
    /// Mean energy from each macronutrient over the logged days that know each value.
    pub macro_energy: MacroEnergy,
    pub nova: NovaShare,
    pub weight: WeightSummary,
    pub weight_change: WeightChange,
    /// Null when the range has no micronutrient coverage.
    pub micronutrients: Option<PeriodCoverage>,
}

#[derive(Debug, Serialize)]
pub struct KnownDays {
    pub energy: usize,
    pub protein: usize,
    pub carbohydrate: usize,
    pub fat: usize,
    pub alcohol: usize,
}

/// kcal per day; null when no day has the value.
#[derive(Debug, Serialize)]
pub struct EnergySummary {
    /// Mean over the logged days that know each value.
    pub eaten_kcal: Option<f64>,
    pub calorie_target_kcal: Option<f64>,
    pub estimated_expenditure_kcal: Option<f64>,
}

/// Grams per day; null when no day has the value.
#[derive(Debug, Serialize)]
pub struct ProteinSummary {
    /// Mean over the logged days that know each value.
    pub eaten_g: Option<f64>,
    pub target_g: Option<f64>,
}

/// kcal per day at 4/4/9/7 kcal per gram, over the logged days that know each value.
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

/// A mean of the days that know this metric; an unknown day never contributes a zero.
#[derive(Clone, Copy, Debug, Default)]
struct Known(Mean);

impl Known {
    fn add(&mut self, value: Option<f64>) {
        self.0.add(value);
    }

    fn per_day(self, factor: f64, places: i32) -> Option<f64> {
        self.0.get().map(|mean| round_to(mean * factor, places))
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
    /// Logged days whose energy is known.
    intake_days: usize,
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
    /// A logged day contributes each known metric independently of its energy.
    fn add_eaten(&mut self, eaten: &Eaten<'_>) {
        self.logged_days += 1;
        self.nova = self.nova + NovaEnergy::of(&eaten.portions);
        let energy = eaten.total(parameters::ENERGY_KCAL);
        self.intake_days += usize::from(energy.is_some());
        self.energy.add(energy);
        self.protein.add(eaten.total(parameters::PROTEIN));
        self.carbs
            .add(eaten.total(parameters::AVAILABLE_CARBOHYDRATE));
        self.fat.add(eaten.total(parameters::FAT));
        self.alcohol.add(eaten.total(parameters::ALCOHOL));
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
            if let Some(eaten) = self.eaten.get(day) {
                sums.add_eaten(eaten);
            }
            if let (Some(tally), Some(coverage)) = (tally.as_mut(), &self.coverage)
                && let Ok(assessment) = coverage.diary.assess(&coverage.person, *day)
            {
                tally.add(&assessment);
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

        let (logged, intake_days) = (sums.logged_days, sums.intake_days);
        let rounded = |mean: Mean, places| mean.get().map(|value| round_to(value, places));
        let period = Period {
            start,
            end,
            days: days.len(),
            partial: calendar_start < start || calendar_end > end,
            logged_days: logged,
            missing_days: days.len() - logged,
            intake_days,
            known_days: KnownDays {
                energy: sums.energy.0.count,
                protein: sums.protein.0.count,
                carbohydrate: sums.carbs.0.count,
                fat: sums.fat.0.count,
                alcohol: sums.alcohol.0.count,
            },
            energy: EnergySummary {
                eaten_kcal: sums.energy.per_day(1.0, 0),
                calorie_target_kcal: rounded(sums.calorie_target, 0),
                estimated_expenditure_kcal: rounded(sums.expenditure, 0),
            },
            protein: ProteinSummary {
                eaten_g: sums.protein.per_day(1.0, 1),
                target_g: rounded(sums.protein_target, 1),
            },
            macro_energy: MacroEnergy {
                protein_kcal: sums.protein.per_day(KCAL_PER_G_PROTEIN, 0),
                carbohydrate_kcal: sums.carbs.per_day(KCAL_PER_G_CARBS, 0),
                fat_kcal: sums.fat.per_day(KCAL_PER_G_FAT, 0),
                alcohol_kcal: sums.alcohol.per_day(KCAL_PER_G_ALCOHOL, 0),
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
    let logs = meal_logs::load(
        db,
        &Selection::Days(from, to),
        Values::Only(&meal_logs::KEY_NUTRIENTS),
    )
    .await?;
    let weigh_ins = targets::weigh_ins(db, to).await?;
    let goals = goals::history(db).await?;

    let portions = || logs.iter().flat_map(|log| &log.portions);
    let mut versions: Vec<&str> = portions().map(StoredPortion::dataset_version).collect();
    versions.sort_unstable();
    versions.dedup();
    let sources = versions.into_iter().map(food_database::source).collect();
    let label_source = portions()
        .any(StoredPortion::is_product)
        .then_some(LABEL_SOURCE);

    let mut eaten: BTreeMap<CalendarDay, Eaten<'_>> = BTreeMap::new();
    for log in &logs {
        let Some(day) = CalendarDay::parse(&log.day) else {
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
        coverage: match unavailable {
            Some(_) => None,
            None => Some(CoverageInputs {
                person,
                diary: Diary::load(db, from, to).await?,
            }),
        },
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
        sources,
        label_source,
        periods,
    })
}
