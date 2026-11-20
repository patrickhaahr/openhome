use crate::services::nutrition::supplements::DoseRequest;
use crate::services::patch::Patch;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(rename_all = "snake_case")]
pub enum ValueSource {
    Food,
    Label,
    Estimate,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PortionRequest {
    #[serde(default)]
    pub food_id: Option<i64>,
    #[serde(default)]
    pub barcode: Option<String>,
    pub grams: f64,
    #[serde(default)]
    pub estimates: BTreeMap<String, f64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LogMeal {
    pub eaten_at: String,
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub portions: Option<Vec<PortionRequest>>,
    #[serde(default)]
    pub supplement_doses: Option<Vec<DoseRequest>>,
    #[serde(default)]
    pub meal: Option<String>,
    #[serde(default)]
    pub overrides: Option<Vec<OverrideRequest>>,
    pub request_id: String,
    #[serde(default)]
    pub photo: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OverrideRequest {
    #[serde(default)]
    pub food_id: Option<i64>,
    #[serde(default)]
    pub barcode: Option<String>,
    #[serde(default)]
    pub grams: Option<f64>,
    #[serde(default)]
    pub supplement_id: Option<String>,
    #[serde(default)]
    pub doses: Option<f64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EditMealLog {
    #[serde(default)]
    pub eaten_at: Patch<String>,
    #[serde(default)]
    pub note: Patch<String>,
    #[serde(default)]
    pub portions: Patch<Vec<PortionRequest>>,
    #[serde(default)]
    pub supplement_doses: Patch<Vec<DoseRequest>>,
    #[serde(default)]
    pub photo: Patch<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MealLogsQuery {
    pub day: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
}
