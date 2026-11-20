use crate::services::nutrition::goals::{self, DatedGoal};
use crate::services::nutrition::meal_logs::MealLogError;
use crate::services::training::calendar::CalendarDay;
use sqlx::SqlitePool;
use std::collections::{BTreeMap, BTreeSet};

mod inputs;
mod macros;
mod weeks;
pub(super) use inputs::weigh_ins;
use inputs::{Body, logged_energy};
pub use inputs::{Missing, Sex, TrendWeight};
pub(super) use macros::round_to;
pub use macros::{
    AvailableTargets, CarbsTarget, ExpenditureBasis, FatFloor, MacroStatus, MacroTargets,
    ProteinTarget, Targets,
};
pub use weeks::Week;
use weeks::{freeze_weeks, stored_weeks};

pub const KCAL_PER_KG: f64 = 7700.0;
const SMOOTHING: f64 = 0.1;
const WINDOW_DAYS: u64 = 28;
const FULL_BLEND_DAYS: usize = 14;
const ACTIVITY_FACTOR: f64 = 1.5;
const PROTEIN_G_PER_KG: f64 = 2.2;
const FAT_FLOOR_G_PER_KG: f64 = 0.7;
pub(super) const KCAL_PER_G_PROTEIN: f64 = 4.0;
pub(super) const KCAL_PER_G_CARBS: f64 = 4.0;
pub(super) const KCAL_PER_G_FAT: f64 = 9.0;

#[derive(Clone, Debug)]
pub struct DayPlan {
    pub week: Week,
    pub trend_weight_kg: f64,
    pub goal: DatedGoal,
    pub calorie_target_kcal: f64,
    pub bmr_floor_applied: bool,
    pub protein_target_g: f64,
    pub fat_floor_g: f64,
    pub carbs_target_g: f64,
}

impl DayPlan {
    pub(super) fn new(week: Week, trend_weight_kg: f64, goal: DatedGoal) -> Self {
        let adjusted = week.expenditure_kcal + goal.goal.weekly_change_kg() * KCAL_PER_KG / 7.0;
        let calorie_target_kcal = adjusted.max(week.bmr_kcal);
        let protein_target_g = PROTEIN_G_PER_KG * trend_weight_kg;
        let fat_floor_g = FAT_FLOOR_G_PER_KG * trend_weight_kg;
        let carbs_target_g = ((calorie_target_kcal
            - KCAL_PER_G_PROTEIN * protein_target_g
            - KCAL_PER_G_FAT * fat_floor_g)
            / KCAL_PER_G_CARBS)
            .max(0.0);
        Self {
            bmr_floor_applied: adjusted < week.bmr_kcal,
            week,
            trend_weight_kg,
            goal,
            calorie_target_kcal,
            protein_target_g,
            fat_floor_g,
            carbs_target_g,
        }
    }
}

#[derive(Clone, Debug)]
pub enum Plan {
    Available(Box<DayPlan>),
    Unavailable {
        day: CalendarDay,
        missing: Vec<Missing>,
    },
}

pub async fn plan(
    db: &SqlitePool,
    from: CalendarDay,
    to: CalendarDay,
    today: CalendarDay,
) -> Result<Vec<(CalendarDay, Plan)>, MealLogError> {
    let monday_of = |day: CalendarDay| day.iso_week_bounds().map_or(day, |(monday, _)| monday);
    let days: Vec<CalendarDay> = crate::services::training::calendar::days(from, to).collect();
    let mut last_read: BTreeMap<CalendarDay, CalendarDay> = BTreeMap::new();
    for day in &days {
        last_read.insert(monday_of(*day), *day);
    }
    let (Some(first_monday), Some(last_monday)) = (
        last_read.keys().next().copied(),
        last_read.keys().next_back().copied(),
    ) else {
        return Ok(Vec::new());
    };

    let trend = TrendWeight::of(&weigh_ins(db, to).await?);
    let goals = goals::history(db).await?;
    let mut weeks = stored_weeks(db, first_monday, last_monday).await?;
    let body = Body::read(db).await?;

    let unstored: BTreeSet<CalendarDay> = last_read
        .keys()
        .filter(|monday| !weeks.contains_key(monday))
        .copied()
        .collect();
    if let (Ok(body), Some(first), Some(last)) =
        (&body, unstored.first().copied(), unstored.last().copied())
    {
        let energy = logged_energy(
            db,
            Week::window(first).map_or(first, |(start, _)| start),
            last.previous(),
        )
        .await?;
        let today_monday = monday_of(today);
        let (eligible, future): (Vec<Week>, Vec<Week>) = unstored
            .into_iter()
            .filter_map(|monday| Week::compute(monday, last_read[&monday], *body, &trend, &energy))
            .partition(|week| week.monday <= today_monday);
        weeks.extend(freeze_weeks(db, &eligible).await?);
        for week in future {
            weeks.insert(week.monday, week);
        }
    }

    Ok(days
        .into_iter()
        .map(|day| {
            let week = weeks.get(&monday_of(day));
            let plan = match (week, trend.on(day)) {
                (Some(week), Some(trend_weight)) => Plan::Available(Box::new(DayPlan::new(
                    week.clone(),
                    trend_weight,
                    goals.on(day),
                ))),
                (week, trend_weight) => {
                    let mut missing = match (week, &body) {
                        (None, Err(missing)) => missing.clone(),
                        _ => Vec::new(),
                    };
                    if trend_weight.is_none() {
                        missing.push(Missing::BodyWeight);
                    }
                    Plan::Unavailable { day, missing }
                }
            };
            (day, plan)
        })
        .collect())
}

pub async fn plan_day(
    db: &SqlitePool,
    day: CalendarDay,
    today: CalendarDay,
) -> Result<Plan, MealLogError> {
    Ok(plan(db, day, day, today).await?.pop().map_or(
        Plan::Unavailable {
            day,
            missing: Vec::new(),
        },
        |(_, plan)| plan,
    ))
}
