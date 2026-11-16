use crate::services::nutrition::supplements::DoseRequest;
use crate::services::patch::Patch;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Where a snapshotted value came from: the food database, a Product label or an agent estimate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(rename_all = "snake_case")]
pub enum ValueSource {
    Food,
    Label,
    Estimate,
}

/// One Portion to log: edible grams of a Food (`food_id`) or of a linked Product (`barcode`).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PortionRequest {
    #[serde(default)]
    pub food_id: Option<i64>,
    #[serde(default)]
    pub barcode: Option<String>,
    pub grams: f64,
    /// Agent estimates per 100 g edible part, keyed and united as in the parameter vocabulary,
    /// for values neither a label nor the Food has.
    #[serde(default)]
    pub estimates: BTreeMap<String, f64>,
}

/// A meal to log: its `portions` and `supplement_doses`, or the name of a saved Meal (`meal`)
/// whose items are copied, with `overrides` replacing the grams of the items they name by
/// `food_id` or `barcode` and the doses of the Supplements they name by `supplement_id`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LogMeal {
    /// RFC 3339 with an offset: when the meal was eaten.
    pub eaten_at: String,
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub portions: Option<Vec<PortionRequest>>,
    #[serde(default)]
    pub supplement_doses: Option<Vec<DoseRequest>>,
    /// A saved Meal's name, in any case.
    #[serde(default)]
    pub meal: Option<String>,
    /// One-off grams or doses for items of `meal`, each naming its item by `food_id`,
    /// `barcode` or `supplement_id`.
    #[serde(default)]
    pub overrides: Option<Vec<OverrideRequest>>,
    pub request_id: String,
    /// A base64 JPEG or PNG of the meal.
    #[serde(default)]
    pub photo: Option<String>,
}

/// One-off grams for a Meal's Food or Product item (`food_id` or `barcode` with `grams`), or
/// one-off doses for its Supplement (`supplement_id` with `doses`).
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

/// Omitted fields keep their value; `note: null` clears the note and `photo: null` removes the
/// photo. `portions` replaces every Portion with new snapshots of the current Products and Food
/// database, and `supplement_doses` every dose with new snapshots of the current Supplements;
/// `photo` (base64 JPEG or PNG) replaces the photo.
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

/// One day, or an inclusive range of days.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MealLogsQuery {
    pub day: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
}
