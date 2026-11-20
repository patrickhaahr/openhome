use super::references::{Group, Reference, Share};
use super::{DayIntake, NutrientDay};
use crate::services::nutrition::meal_logs::round;
use crate::services::nutrition::targets::Missing;
use crate::services::training::calendar::CalendarDay;
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LimitStatus {
    #[serde(rename = "none")]
    Clear,
    Caution,
    CutDown,
}

#[derive(Debug, Serialize)]
pub struct UpperLimitStatus {
    pub applies_to: Share,
    pub limit: f64,
    pub amount: Option<f64>,
    pub seven_day_average: Option<f64>,
    pub averaged_days: usize,
    pub status: LimitStatus,
    pub source: &'static str,
}

#[derive(Debug, Serialize)]
pub struct NutrientStatus {
    pub key: &'static str,
    pub name: &'static str,
    pub group: Group,
    pub unit: &'static str,
    pub amount: Option<f64>,
    pub food: Option<f64>,
    pub supplements: Option<f64>,
    pub unknown_portions: usize,
    pub recommended_intake: f64,
    pub adequate_intake: bool,
    pub percent_of_recommended: Option<f64>,
    pub source: &'static str,
    pub upper_limits: Vec<UpperLimitStatus>,
    pub upper_limit_status: Option<LimitStatus>,
}

impl NutrientStatus {
    pub(super) fn new(
        reference: &'static Reference,
        day: NutrientDay,
        intake: Option<&DayIntake>,
    ) -> Self {
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

#[derive(Debug, Serialize)]
pub struct TrailingWindow {
    pub from: CalendarDay,
    pub to: CalendarDay,
    pub logged_days: usize,
}

#[derive(Debug, Serialize)]
pub struct Citation {
    pub name: &'static str,
    pub citation: &'static str,
}

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
    pub age: u32,
    pub age_band: &'static str,
    pub reference_energy_mj: f64,
    pub trailing_window: TrailingWindow,
    pub nutrients: Vec<NutrientStatus>,
    pub sources: &'static [Citation],
}

#[derive(Debug, Serialize)]
pub struct Unavailable {
    pub missing: Vec<Missing>,
    pub reason: String,
}

#[derive(Debug, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Micronutrients {
    Available(Box<Panel>),
    Unavailable(Unavailable),
}
