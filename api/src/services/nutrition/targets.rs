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

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use sqlx::SqlitePool;

use super::goals::{self, DatedGoal};
use super::intake::Intake;
use super::meal_logs::{self, KeyNutrients, MealLogError, Selection, StoredPortion, Total};
use super::parameters;
use super::supplements::StoredDose;
use crate::services::training::calendar::CalendarDay;

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
const KCAL_PER_G_PROTEIN: f64 = 4.0;
const KCAL_PER_G_CARBS: f64 = 4.0;
const KCAL_PER_G_FAT: f64 = 9.0;

/// Trend Weight by day: the moving average after each weigh-in, in date order.
#[derive(Clone, Debug, Default)]
pub struct TrendWeight {
    points: Vec<(CalendarDay, f64)>,
}

impl TrendWeight {
    /// Smooths weigh-ins given in date order, one per day; the first one seeds the average.
    #[must_use]
    pub fn of(weigh_ins: &[(CalendarDay, f64)]) -> Self {
        let mut trend: Option<f64> = None;
        let points = weigh_ins
            .iter()
            .map(|(day, weight)| {
                let next = trend.map_or(*weight, |trend| trend + SMOOTHING * (weight - trend));
                trend = Some(next);
                (*day, next)
            })
            .collect();
        Self { points }
    }

    /// The Trend Weight on `day`, carried from the last weigh-in on or before it; `None` before
    /// the first weigh-in.
    #[must_use]
    pub fn on(&self, day: CalendarDay) -> Option<f64> {
        let after = self.points.partition_point(|(weighed, _)| *weighed <= day);
        after.checked_sub(1).map(|index| self.points[index].1)
    }

    /// The first weigh-in, which seeds the average.
    fn first(&self) -> Option<(CalendarDay, f64)> {
        self.points.first().copied()
    }

    /// The first weigh-in from `from` through `to`, with its Trend Weight.
    fn first_between(&self, from: CalendarDay, to: CalendarDay) -> Option<(CalendarDay, f64)> {
        self.points
            .iter()
            .find(|(day, _)| *day >= from && *day <= to)
            .copied()
    }
}

/// Sex as the Mifflin–St Jeor formula and the NNR 2023 references take it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sex {
    Male,
    Female,
}

impl Sex {
    /// `male` or `female` in any case; the Profile stores free text, and nothing else is guessed.
    pub(super) fn parse(text: &str) -> Option<Self> {
        match text.trim().to_lowercase().as_str() {
            "male" => Some(Self::Male),
            "female" => Some(Self::Female),
            _ => None,
        }
    }
}

/// Data a target needs and does not have.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Missing {
    Height,
    Sex,
    BirthDate,
    BodyWeight,
}

/// The Profile values the BMR formula needs.
#[derive(Clone, Copy, Debug)]
struct Body {
    height_cm: f64,
    sex: Sex,
    birth_date: CalendarDay,
}

impl Body {
    /// The Profile's values, or what it lacks.
    async fn read(db: &SqlitePool) -> Result<Result<Self, Vec<Missing>>, sqlx::Error> {
        let row = sqlx::query!(
            r#"SELECT height_cm, sex, birth_date AS "birth_date?" FROM profile WHERE id = 1"#
        )
        .fetch_optional(db)
        .await?;
        let (height, sex, birth) = row.map_or((None, None, None), |row| {
            (row.height_cm, row.sex, row.birth_date)
        });
        let height_cm = height.filter(|height| *height > 0.0);
        let sex = sex.as_deref().and_then(Sex::parse);
        let birth_date = birth.as_deref().and_then(CalendarDay::parse);
        Ok(match (height_cm, sex, birth_date) {
            (Some(height_cm), Some(sex), Some(birth_date)) => Ok(Self {
                height_cm,
                sex,
                birth_date,
            }),
            _ => Err([
                (height_cm.is_none(), Missing::Height),
                (sex.is_none(), Missing::Sex),
                (birth_date.is_none(), Missing::BirthDate),
            ]
            .into_iter()
            .filter_map(|(missing, what)| missing.then_some(what))
            .collect()),
        })
    }

    /// Mifflin–St Jeor resting energy in kcal per day at `weight_kg` on `day`.
    fn bmr(self, weight_kg: f64, day: CalendarDay) -> Option<f64> {
        let age = f64::from(day.years_since(self.birth_date)?);
        let offset = match self.sex {
            Sex::Male => 5.0,
            Sex::Female => -161.0,
        };
        Some(10.0 * weight_kg + 6.25 * self.height_cm - 5.0 * age + offset)
    }
}

/// A week's Estimated Expenditure and BMR, and how they were found.
#[derive(Clone, Debug)]
pub struct Week {
    pub monday: CalendarDay,
    pub expenditure_kcal: f64,
    pub bmr_kcal: f64,
    pub prior_kcal: f64,
    pub data_estimate_kcal: Option<f64>,
    pub logged_days: usize,
    pub blend_weight: f64,
    pub trend_weight_kg: f64,
    pub trend_weight_day: CalendarDay,
    /// Stored, so it no longer changes; false for a week starting after today.
    pub fixed: bool,
}

impl Week {
    /// The 28 days before Monday.
    fn window(monday: CalendarDay) -> Option<(CalendarDay, CalendarDay)> {
        Some((monday.checked_sub_days(WINDOW_DAYS)?, monday.previous()))
    }

    /// The week of `monday` from the data before it, or `None` while no weigh-in gives it a
    /// Trend Weight up to `last_read`, the latest day of the week being read.
    fn compute(
        monday: CalendarDay,
        last_read: CalendarDay,
        body: Body,
        trend: &TrendWeight,
        energy: &BTreeMap<CalendarDay, f64>,
    ) -> Option<Self> {
        let (window_start, window_end) = Self::window(monday)?;
        let (trend_weight_day, trend_weight_kg) = trend
            .on(window_end)
            .map(|weight| (window_end, weight))
            .or_else(|| trend.first_between(monday, last_read))?;
        let bmr_kcal = body.bmr(trend_weight_kg, monday)?;
        let prior_kcal = bmr_kcal * ACTIVITY_FACTOR;

        let logged: Vec<f64> = energy
            .range(window_start..=window_end)
            .map(|(_, kcal)| *kcal)
            .collect();
        let data_estimate_kcal = Self::data_estimate(window_start, window_end, trend, &logged);
        let blend_weight = if data_estimate_kcal.is_some() {
            logged.len().min(FULL_BLEND_DAYS) as f64 / FULL_BLEND_DAYS as f64
        } else {
            0.0
        };
        let expenditure_kcal = data_estimate_kcal.map_or(prior_kcal, |data| {
            blend_weight * data + (1.0 - blend_weight) * prior_kcal
        });
        (bmr_kcal > 0.0 && expenditure_kcal > 0.0).then_some(Self {
            monday,
            expenditure_kcal,
            bmr_kcal,
            prior_kcal,
            data_estimate_kcal,
            logged_days: logged.len(),
            blend_weight,
            trend_weight_kg,
            trend_weight_day,
            fixed: false,
        })
    }

    /// Mean logged energy minus the energy of the Trend Weight change over the window: from the
    /// day before it, or from the first weigh-in when that falls inside it.
    fn data_estimate(
        window_start: CalendarDay,
        window_end: CalendarDay,
        trend: &TrendWeight,
        logged: &[f64],
    ) -> Option<f64> {
        if logged.is_empty() {
            return None;
        }
        let before = window_start.previous();
        let (start_day, start) = trend
            .on(before)
            .map(|weight| (before, weight))
            .or_else(|| trend.first().filter(|(day, _)| *day >= window_start))?;
        let end = trend.on(window_end)?;
        let days = start_day.days_until(window_end);
        if days < 1 {
            return None;
        }
        let mean = logged.iter().sum::<f64>() / logged.len() as f64;
        Some(mean - (end - start) * KCAL_PER_KG / days as f64)
    }
}

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
    fn new(week: Week, trend_weight_kg: f64, goal: DatedGoal) -> Self {
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

/// Each day's targets from `from` through `to`, in order. Writes the weekly target record of each
/// week that has none yet and starts on or before `today`.
pub async fn plan(
    db: &SqlitePool,
    from: CalendarDay,
    to: CalendarDay,
    today: CalendarDay,
) -> Result<Vec<(CalendarDay, Plan)>, MealLogError> {
    let monday_of = |day: CalendarDay| day.iso_week_bounds().map_or(day, |(monday, _)| monday);
    let days: Vec<CalendarDay> = crate::services::training::calendar::days(from, to).collect();
    // The latest day read in each week.
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
        for monday in unstored {
            let Some(mut week) = Week::compute(monday, last_read[&monday], *body, &trend, &energy)
            else {
                continue;
            };
            if monday <= today_monday {
                week = store_week(db, &week).await?;
            }
            weeks.insert(monday, week);
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

/// Body Weight up to `to`, in date order.
async fn weigh_ins(
    db: &SqlitePool,
    to: CalendarDay,
) -> Result<Vec<(CalendarDay, f64)>, sqlx::Error> {
    let to = to.to_string();
    let rows = sqlx::query!(
        r#"SELECT CAST(date AS TEXT) AS "date!: String", weight_kg FROM body_weight
        WHERE date <= ?1 ORDER BY date"#,
        to
    )
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .filter_map(|row| Some((CalendarDay::parse(&row.date)?, row.weight_kg)))
        .collect())
}

/// Each day's known logged energy from `from` through `to`, Portions and Supplement Doses alike;
/// days without Meal Logs, or whose logged energy is all unknown, are left out.
async fn logged_energy(
    db: &SqlitePool,
    from: CalendarDay,
    to: CalendarDay,
) -> Result<BTreeMap<CalendarDay, f64>, MealLogError> {
    if to < from {
        return Ok(BTreeMap::new());
    }
    let logs = meal_logs::load(db, &Selection::Days(from, to), false).await?;
    let mut by_day: BTreeMap<CalendarDay, Intake> = BTreeMap::new();
    for log in &logs {
        let Some(day) = CalendarDay::parse(&log.day) else {
            continue;
        };
        let portions: Vec<&StoredPortion> = log.portions.iter().collect();
        let doses: Vec<&StoredDose> = log.doses.iter().collect();
        let intake = Intake::of(parameters::ENERGY_KCAL, &portions, &doses);
        let total = by_day.remove(&day).unwrap_or_default() + intake;
        by_day.insert(day, total);
    }
    Ok(by_day
        .into_iter()
        .filter_map(|(day, intake)| Some((day, intake.total()?)))
        .collect())
}

/// The stored weekly records for Mondays from `first` through `last`.
async fn stored_weeks(
    db: &SqlitePool,
    first: CalendarDay,
    last: CalendarDay,
) -> Result<BTreeMap<CalendarDay, Week>, sqlx::Error> {
    let (first, last) = (first.to_string(), last.to_string());
    let rows = sqlx::query!(
        r#"SELECT monday, expenditure_kcal, bmr_kcal, prior_kcal, data_estimate_kcal,
            logged_days, blend_weight, trend_weight_kg, trend_weight_day
        FROM nutrition_weeks WHERE monday BETWEEN ?1 AND ?2"#,
        first,
        last
    )
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|row| {
            let day = |text: &str| {
                CalendarDay::parse(text)
                    .ok_or_else(|| sqlx::Error::Decode(format!("invalid week day {text}").into()))
            };
            let monday = day(&row.monday)?;
            Ok((
                monday,
                Week {
                    monday,
                    expenditure_kcal: row.expenditure_kcal,
                    bmr_kcal: row.bmr_kcal,
                    prior_kcal: row.prior_kcal,
                    data_estimate_kcal: row.data_estimate_kcal,
                    logged_days: usize::try_from(row.logged_days).unwrap_or_default(),
                    blend_weight: row.blend_weight,
                    trend_weight_kg: row.trend_weight_kg,
                    trend_weight_day: day(&row.trend_weight_day)?,
                    fixed: true,
                },
            ))
        })
        .collect()
}

/// Stores `week` unless a concurrent read stored it first, and returns the stored record.
async fn store_week(db: &SqlitePool, week: &Week) -> Result<Week, sqlx::Error> {
    let monday = week.monday.to_string();
    let trend_weight_day = week.trend_weight_day.to_string();
    let logged_days = week.logged_days as i64;
    sqlx::query!(
        r#"INSERT INTO nutrition_weeks (monday, expenditure_kcal, bmr_kcal, prior_kcal,
            data_estimate_kcal, logged_days, blend_weight, trend_weight_kg, trend_weight_day)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9) ON CONFLICT (monday) DO NOTHING"#,
        monday,
        week.expenditure_kcal,
        week.bmr_kcal,
        week.prior_kcal,
        week.data_estimate_kcal,
        logged_days,
        week.blend_weight,
        week.trend_weight_kg,
        trend_weight_day
    )
    .execute(db)
    .await?;
    stored_weeks(db, week.monday, week.monday)
        .await?
        .remove(&week.monday)
        .ok_or(sqlx::Error::RowNotFound)
}

/// A macro eaten against its target or floor.
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MacroStatus {
    /// At or above the target.
    Met,
    /// Below it: protein still to eat, or fat below its floor.
    Below,
}

#[derive(Debug, Serialize)]
pub struct ProteinTarget {
    pub target_g: f64,
    pub eaten_g: f64,
    pub status: MacroStatus,
}

#[derive(Debug, Serialize)]
pub struct FatFloor {
    pub floor_g: f64,
    pub eaten_g: f64,
    pub status: MacroStatus,
}

/// Shown, never graded.
#[derive(Debug, Serialize)]
pub struct CarbsTarget {
    pub target_g: f64,
    pub eaten_g: f64,
}

#[derive(Debug, Serialize)]
pub struct MacroTargets {
    pub protein: ProteinTarget,
    pub fat: FatFloor,
    pub carbs: CarbsTarget,
}

/// How the week's Estimated Expenditure was found.
#[derive(Debug, Serialize)]
pub struct ExpenditureBasis {
    pub window_start: String,
    pub window_end: String,
    pub logged_days: usize,
    pub data_estimate_kcal: Option<f64>,
    pub prior_kcal: f64,
    pub blend_weight: f64,
}

/// A day's targets beside what was eaten, rounded for display: kcal to whole numbers, grams to
/// tenths and kg to hundredths.
#[derive(Debug, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Targets {
    Available(Box<AvailableTargets>),
    Unavailable {
        missing: Vec<Missing>,
        reason: String,
    },
}

#[derive(Debug, Serialize)]
pub struct AvailableTargets {
    pub week_start: String,
    pub week_fixed: bool,
    pub trend_weight_kg: f64,
    pub bmr_kcal: f64,
    pub estimated_expenditure_kcal: f64,
    pub expenditure_basis: ExpenditureBasis,
    pub goal: DatedGoal,
    pub calorie_target_kcal: f64,
    pub bmr_floor_applied: bool,
    pub eaten_kcal: f64,
    pub remaining_kcal: f64,
    pub macros: MacroTargets,
}

fn round_to(value: f64, places: i32) -> f64 {
    let scale = 10f64.powi(places);
    (value * scale).round() / scale
}

fn status(eaten: f64, target: f64) -> MacroStatus {
    if eaten >= target {
        MacroStatus::Met
    } else {
        MacroStatus::Below
    }
}

impl Targets {
    /// `plan` beside the day's `totals`; nothing eaten counts as zero.
    #[must_use]
    pub fn new(plan: Plan, totals: &KeyNutrients<Total>) -> Self {
        let plan = match plan {
            Plan::Available(plan) => plan,
            Plan::Unavailable { day, missing } => {
                return Self::Unavailable {
                    reason: reason(day, &missing),
                    missing,
                };
            }
        };
        let eaten = |total: &Total| total.value.unwrap_or(0.0);
        let calorie_target_kcal = round_to(plan.calorie_target_kcal, 0);
        let eaten_kcal = round_to(eaten(&totals.energy_kcal), 0);
        let protein_target = round_to(plan.protein_target_g, 1);
        let fat_floor = round_to(plan.fat_floor_g, 1);
        let protein_eaten = round_to(eaten(&totals.protein), 1);
        let fat_eaten = round_to(eaten(&totals.fat), 1);
        let week = &plan.week;
        let (window_start, window_end) = Week::window(week.monday)
            .map_or((String::new(), String::new()), |(start, end)| {
                (start.to_string(), end.to_string())
            });
        Self::Available(Box::new(AvailableTargets {
            week_start: week.monday.to_string(),
            week_fixed: week.fixed,
            trend_weight_kg: round_to(plan.trend_weight_kg, 2),
            bmr_kcal: round_to(week.bmr_kcal, 0),
            estimated_expenditure_kcal: round_to(week.expenditure_kcal, 0),
            expenditure_basis: ExpenditureBasis {
                window_start,
                window_end,
                logged_days: week.logged_days,
                data_estimate_kcal: week.data_estimate_kcal.map(|kcal| round_to(kcal, 0)),
                prior_kcal: round_to(week.prior_kcal, 0),
                blend_weight: round_to(week.blend_weight, 4),
            },
            goal: plan.goal,
            calorie_target_kcal,
            bmr_floor_applied: plan.bmr_floor_applied,
            eaten_kcal,
            remaining_kcal: calorie_target_kcal - eaten_kcal,
            macros: MacroTargets {
                protein: ProteinTarget {
                    target_g: protein_target,
                    eaten_g: protein_eaten,
                    status: status(protein_eaten, protein_target),
                },
                fat: FatFloor {
                    floor_g: fat_floor,
                    eaten_g: fat_eaten,
                    status: status(fat_eaten, fat_floor),
                },
                carbs: CarbsTarget {
                    target_g: round_to(plan.carbs_target_g, 1),
                    eaten_g: round_to(eaten(&totals.available_carbohydrate), 1),
                },
            },
        }))
    }
}

/// Why a day has no targets, naming what to add.
fn reason(day: CalendarDay, missing: &[Missing]) -> String {
    let profile: Vec<&str> = missing
        .iter()
        .filter_map(|missing| match missing {
            Missing::Height => Some("height"),
            Missing::Sex => Some("sex (male or female)"),
            Missing::BirthDate => Some("birth date"),
            Missing::BodyWeight => None,
        })
        .collect();
    let profile = match profile.as_slice() {
        [] => None,
        [only] => Some(format!("the Profile's {only}")),
        [rest @ .., last] => Some(format!("the Profile's {} and {last}", rest.join(", "))),
    };
    let weight = missing
        .contains(&Missing::BodyWeight)
        .then(|| format!("a Body Weight on or before {day}"));
    match (profile, weight) {
        (Some(profile), Some(weight)) => format!("Targets need {profile}, and {weight}"),
        (Some(needed), None) | (None, Some(needed)) => format!("Targets need {needed}"),
        (None, None) => "Targets are unavailable for this day".to_owned(),
    }
}
