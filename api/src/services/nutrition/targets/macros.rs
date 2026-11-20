use super::{Missing, Plan, Week};
use crate::services::nutrition::goals::DatedGoal;
use crate::services::nutrition::meal_logs::{KeyNutrients, Total};
use crate::services::training::calendar::CalendarDay;
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MacroStatus {
    Met,
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

#[derive(Debug, Serialize)]
pub struct ExpenditureBasis {
    pub window_start: String,
    pub window_end: String,
    pub logged_days: usize,
    pub data_estimate_kcal: Option<f64>,
    pub prior_kcal: f64,
    pub blend_weight: f64,
}

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

pub(in crate::services::nutrition) fn round_to(value: f64, places: i32) -> f64 {
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
