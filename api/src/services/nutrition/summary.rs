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

pub const MAX_SUMMARY_DAYS: i64 = 365;
const KCAL_PER_G_ALCOHOL: f64 = 7.0;
const WEEK_DAYS: usize = 7;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Aggregation {
    Day,
    Week,
    Month,
}

impl Aggregation {
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
    pub logged_days: usize,
    pub missing_days: usize,
    pub micronutrients: MicronutrientCoverage,
    pub sources: Vec<Source>,
    pub label_source: Option<LabelSource>,
    pub periods: Vec<Period>,
}

#[derive(Debug, Serialize)]
pub struct Period {
    pub start: CalendarDay,
    pub end: CalendarDay,
    pub days: usize,
    pub partial: bool,
    pub logged_days: usize,
    pub missing_days: usize,
    pub intake_days: usize,
    pub known_days: KnownDays,
    pub energy: EnergySummary,
    pub protein: ProteinSummary,
    pub macro_energy: MacroEnergy,
    pub nova: NovaShare,
    pub weight: WeightSummary,
    pub weight_change: WeightChange,
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

#[derive(Debug, Serialize)]
pub struct EnergySummary {
    pub eaten_kcal: Option<f64>,
    pub calorie_target_kcal: Option<f64>,
    pub estimated_expenditure_kcal: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct ProteinSummary {
    pub eaten_g: Option<f64>,
    pub target_g: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct MacroEnergy {
    pub protein_kcal: Option<f64>,
    pub carbohydrate_kcal: Option<f64>,
    pub fat_kcal: Option<f64>,
    pub alcohol_kcal: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct WeightSummary {
    pub trend_weight_kg: Option<f64>,
    pub scale_weight_kg: Option<f64>,
    pub weigh_ins: usize,
}

#[derive(Debug, Serialize)]
pub struct WeightChange {
    pub kg_per_week: Option<f64>,
    pub goal_kg_per_week: f64,
}

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

#[derive(Default)]
struct Sums {
    logged_days: usize,
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

struct CoverageInputs {
    person: Result<Person, Vec<Missing>>,
    diary: Diary,
}

struct Inputs<'a> {
    aggregation: Aggregation,
    eaten: BTreeMap<CalendarDay, Eaten<'a>>,
    weigh_ins: BTreeMap<CalendarDay, f64>,
    trend: TrendWeight,
    goals: GoalHistory,
    coverage: Option<CoverageInputs>,
}

impl Inputs<'_> {
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

    fn weekly_change(&self, end: CalendarDay, days: usize) -> Option<f64> {
        let span = days.max(WEEK_DAYS);
        let before = end.checked_sub_days(u64::try_from(span).ok()?)?;
        let change = self.trend.on(end)? - self.trend.on(before)?;
        Some(round_to(change * WEEK_DAYS as f64 / span as f64, 2))
    }
}

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
