//! The target engine (`CONTEXT.md`, Trend Weight, Estimated Expenditure, Calorie Target, Macro
//! Targets; ADR 0014): from Body Weight, the Profile, logged intake and the Nutrition Goal history,
//! each Copenhagen day's Calorie Target and Macro Targets.
//!
//! - **Trend Weight** is an exponential moving average of Body Weight with a smoothing factor of
//!   0.1 per weigh-in. It starts at the first weigh-in and is carried unchanged across days
//!   without one. Body Weight holds one weigh-in per date, so a day never has several.
//! - **Estimated Expenditure** is fixed for each ISO week (the weekly target record,
//!   `nutrition_weeks`) from the 28 days before its Monday. The data estimate is the mean energy
//!   of the days with known logged energy, minus the Trend Weight change × 7,700 kcal ÷ the days it
//!   spans; it is blended at `min(logged days, 14) / 14` with a prior of the Mifflin–St Jeor BMR ×
//!   1.5. The BMR uses the Trend Weight on the Sunday before, or the week's first weigh-in when
//!   nothing was weighed before, the Profile's height and sex, and the age on Monday.
//! - A week's record is written on the first read of any of its days, past weeks included, and
//!   never changed, so later backfills, weigh-ins and Profile edits never rewrite a week's
//!   Calorie Target. Weeks starting after today are computed but not stored.
//! - **Calorie Target** = the week's expenditure + the goal's weekly change × 7,700 ÷ 7 for the
//!   goal in effect that day, never below the week's BMR.
//! - **Macro Targets** follow the day's Trend Weight: protein 2.2 g/kg, met at or above it; a fat
//!   floor of 0.7 g/kg; carbs the remaining energy at 4/4/9 kcal per gram, never below zero.
//!
//! [`plan`] evaluates any range of days with one read of each input, so a summary over a year
//! costs the same handful of queries as one day.
//!
//! Weekly lifecycle ownership stays here: row presence in `nutrition_weeks` is the frozen
//! boundary. `Week::compute` makes a provisional value; `plan` admits only computable,
//! unstored weeks that have started to `freeze_weeks`, which returns the database winner.
//! `Week::fixed` reports that provenance; it is not a separately persisted lifecycle state.

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

/// Energy stored or released per kg of body weight change.
pub const KCAL_PER_KG: f64 = 7700.0;
/// The Trend Weight's smoothing factor per weigh-in.
const SMOOTHING: f64 = 0.1;
/// Days before Monday the Estimated Expenditure looks back over.
const WINDOW_DAYS: u64 = 28;
/// Logged days at which the data estimate takes the whole weight.
const FULL_BLEND_DAYS: usize = 14;
/// The prior's activity multiplier on BMR.
const ACTIVITY_FACTOR: f64 = 1.5;
const PROTEIN_G_PER_KG: f64 = 2.2;
const FAT_FLOOR_G_PER_KG: f64 = 0.7;
pub(super) const KCAL_PER_G_PROTEIN: f64 = 4.0;
pub(super) const KCAL_PER_G_CARBS: f64 = 4.0;
pub(super) const KCAL_PER_G_FAT: f64 = 9.0;

/// One day's targets before what was eaten, unrounded.
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

/// A day's targets, or what they need.
#[derive(Clone, Debug)]
pub enum Plan {
    Available(Box<DayPlan>),
    Unavailable {
        day: CalendarDay,
        missing: Vec<Missing>,
    },
}

/// Each day's targets from `from` through `to`, in order. Freezes each computable, unstored
/// week starting on or before `today`; future weeks remain provisional. Existing rows bypass
/// computation even if today's Profile no longer supplies the inputs a new week would need.
pub async fn plan(
    db: &SqlitePool,
    from: CalendarDay,
    to: CalendarDay,
    today: CalendarDay,
) -> Result<Vec<(CalendarDay, Plan)>, MealLogError> {
    let monday_of = |day: CalendarDay| day.iso_week_bounds().map_or(day, |(monday, _)| monday);
    let days: Vec<CalendarDay> = crate::services::training::calendar::days(from, to).collect();
    // The latest requested day in each week, not today or the week's Sunday. This bounds
    // the first-weigh-in fallback even when a Summary reads several days at once.
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
        // Eligibility belongs to the ISO week, not the requested day: even a future day
        // in the current week freezes its computable record. Missing inputs freeze nothing.
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
                    // A stored week keeps its target whatever the Profile says now.
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

/// One day's targets.
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
