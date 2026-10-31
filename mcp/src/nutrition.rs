//! Food database, NOVA Class, Product, Meal, Supplement and Meal Log tool arguments, forwarded
//! unchanged to Axum's nutrition routes.

use std::collections::BTreeMap;

use rmcp::schemars;
use serde::{Deserialize, Serialize};

use super::patch::Patch;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SearchFoodsRequest {
    /// Words of a Food's Danish or English name, e.g. `skyr`, `kyllingebryst` or `chicken breast`.
    pub query: String,
    /// Foods to return, 1-50; 20 when omitted.
    pub limit: Option<u8>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct FoodRequest {
    /// The Food's `food_id`, as `search_foods` lists it.
    pub food_id: u32,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ClassifyFoodRequest {
    /// The Food's `food_id`, as `search_foods` lists it.
    #[serde(skip_serializing)]
    pub food_id: u32,
    /// The Food's NOVA Class: 1 unprocessed or minimally processed, 2 processed culinary
    /// ingredient, 3 processed, 4 ultra-processed.
    pub nova_class: u8,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LookupProductRequest {
    /// The 8, 12, 13 or 14 digits printed under the package's bars.
    pub barcode: String,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LinkProductRequest {
    /// The package's barcode digits, as given to `lookup_product`.
    pub barcode: String,
    /// The `food_id` of the closest Food, from `suggested_foods` or `search_foods`.
    pub food_id: u32,
    /// Unique ID for this link, 1-128 characters. Reuse the identical arguments and ID after an
    /// uncertain response.
    pub request_id: String,
}

/// One Portion: edible grams of one Food (`food_id`) or of one linked Product (`barcode`).
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PortionRequest {
    /// The Food's `food_id` from `search_foods`. Give either this or `barcode`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub food_id: Option<u32>,
    /// The barcode of a Product linked with `link_product`. Give either this or `food_id`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub barcode: Option<String>,
    /// Edible grams eaten, above 0 and at most 5000; convert millilitres and gross weights first.
    pub grams: f64,
    /// Optional estimates per 100 g edible part for values no source has, keyed and united as
    /// the Food database's parameters (e.g. `{"dietary_fibre": 2.5}` in g). A key the label or
    /// Food already has is refused.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimates: Option<BTreeMap<String, f64>>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LogMealRequest {
    /// When it was eaten: RFC 3339 with an offset, e.g. `2026-10-30T08:15:00+01:00`. Use the
    /// message's send time unless the user describes another time ("an hour ago", "yesterday
    /// morning"); never a future time.
    pub eaten_at: String,
    /// Optional context, at most 500 characters, e.g. "dinner at a friend's" or "rough estimate".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// Up to 50 Portions: a mixed dish split into its Foods. Give these and/or
    /// `supplement_doses`, or `meal`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub portions: Option<Vec<PortionRequest>>,
    /// Up to 50 Supplement Doses taken with this meal, each Supplement once. Give these and/or
    /// `portions`, or `meal`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supplement_doses: Option<Vec<DoseRequest>>,
    /// The name of a saved Meal, in any case, to log its items as Portions and Supplement Doses
    /// without analysing a photo. Give either this or `portions`/`supplement_doses`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meal: Option<String>,
    /// Only with `meal`: one-off amounts for some of its items, each naming its item by the
    /// `food_id`, `barcode` or `supplement_id` that `meals` lists, e.g.
    /// `[{"food_id": 1693, "grams": 200}, {"supplement_id": "...", "doses": 2}]`. The Meal keeps
    /// its own amounts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overrides: Option<Vec<OverrideRequest>>,
    /// Unique ID for this intended meal, 1-128 characters. Reuse the identical arguments and ID
    /// after an uncertain response.
    pub request_id: String,
    /// Optional photo of the meal: the user's JPEG or PNG file as base64, at most 10 MB before
    /// encoding. Stored without its metadata as AVIF with a thumbnail.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub photo: Option<String>,
}

/// One-off amount for one item of a Meal: `food_id` or `barcode` with `grams`, or
/// `supplement_id` with `doses`.
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OverrideRequest {
    /// The Meal item's `food_id`, with `grams`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub food_id: Option<u32>,
    /// The Meal item's `barcode`, with `grams`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub barcode: Option<String>,
    /// Edible grams for the named Food or Product, above 0 and at most 5000.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grams: Option<f64>,
    /// The Meal's Supplement Dose `supplement_id`, with `doses`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supplement_id: Option<String>,
    /// Doses for the named Supplement, above 0 and at most 100.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub doses: Option<f64>,
}

/// One Supplement Dose: how many doses (its tablet, capsule, scoop or ml) of one Supplement.
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DoseRequest {
    /// The Supplement's `id` from `supplements` or `add_supplement`.
    pub supplement_id: String,
    /// Doses taken, above 0 and at most 100; halves are fine, and for a Supplement dosed in ml
    /// it is the millilitres.
    pub doses: f64,
}

/// One Meal item: edible grams of one Food (`food_id`) or one linked Product (`barcode`).
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MealItemRequest {
    /// The Food's `food_id` from `search_foods`. Give either this or `barcode`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub food_id: Option<u32>,
    /// The barcode of a Product linked with `link_product`. Give either this or `food_id`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub barcode: Option<String>,
    /// Edible grams, above 0 and at most 5000.
    pub grams: f64,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SaveMealRequest {
    /// The name the user gave the Meal, 1-100 characters, unique in any case, e.g. "protein
    /// oats".
    pub name: String,
    /// Up to 50 Food or Product items in the user's order, each once. A Meal holds at least one
    /// item or Supplement Dose.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub portions: Option<Vec<MealItemRequest>>,
    /// Up to 50 Supplement Doses in the user's order, each Supplement once, e.g. a morning
    /// supplement stack.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supplement_doses: Option<Vec<DoseRequest>>,
    /// Unique ID for this Meal, 1-128 characters. Reuse the identical arguments and ID after an
    /// uncertain response.
    pub request_id: String,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateMealRequest {
    /// The Meal's `id` from `meals` or `save_meal`.
    #[serde(skip_serializing)]
    pub meal_id: String,
    /// A new name, unique in any case. Omit to preserve; cannot be null.
    #[serde(default, skip_serializing_if = "Patch::missing")]
    pub name: Patch<String>,
    /// The complete replacement list of up to 50 Food or Product items. Omit to preserve;
    /// cannot be null.
    #[serde(default, skip_serializing_if = "Patch::missing")]
    pub portions: Patch<Vec<MealItemRequest>>,
    /// The complete replacement list of up to 50 Supplement Doses. Omit to preserve; cannot be
    /// null.
    #[serde(default, skip_serializing_if = "Patch::missing")]
    pub supplement_doses: Patch<Vec<DoseRequest>>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MealRequest {
    /// The Meal's `id` from `meals` or `save_meal`.
    pub meal_id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MealLogsRequest {
    /// One Europe/Copenhagen day, YYYY-MM-DD. Give either `day` or both `from` and `to`.
    pub day: Option<String>,
    /// First Europe/Copenhagen day of a range, YYYY-MM-DD, included.
    pub from: Option<String>,
    /// Last Europe/Copenhagen day of a range, YYYY-MM-DD, included (at most 90 days in total).
    pub to: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateMealLogRequest {
    /// The Meal Log's `id` from `meal_logs` or `log_meal`.
    #[serde(skip_serializing)]
    pub meal_log_id: String,
    /// Corrected RFC 3339 time with an offset; the Meal Log moves to that time's day. Omit to
    /// preserve; cannot be null.
    #[serde(default, skip_serializing_if = "Patch::missing")]
    pub eaten_at: Patch<String>,
    /// New note; null clears it. Omit to preserve.
    #[serde(default, skip_serializing_if = "Patch::missing")]
    pub note: Patch<String>,
    /// The complete replacement list of up to 50 Portions, snapshotted afresh from the current
    /// Food database. Omit to preserve; cannot be null.
    #[serde(default, skip_serializing_if = "Patch::missing")]
    pub portions: Patch<Vec<PortionRequest>>,
    /// The complete replacement list of up to 50 Supplement Doses, snapshotted afresh from the
    /// current Supplements. Omit to preserve; cannot be null.
    #[serde(default, skip_serializing_if = "Patch::missing")]
    pub supplement_doses: Patch<Vec<DoseRequest>>,
    /// A replacement photo: a base64 JPEG or PNG, at most 10 MB before encoding; null removes
    /// the photo. Omit to preserve.
    #[serde(default, skip_serializing_if = "Patch::missing")]
    pub photo: Patch<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MealLogRequest {
    /// The Meal Log's `id` from `meal_logs` or `log_meal`.
    pub meal_log_id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NutritionDayRequest {
    /// The Europe/Copenhagen calendar day, written YYYY-MM-DD.
    pub day: String,
}

/// What one dose of a Supplement is.
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DoseUnit {
    Tablet,
    Capsule,
    Scoop,
    Ml,
}

/// One dose's amount of a Food database parameter, as the label states it.
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AmountRequest {
    /// A Food database parameter key, e.g. `vitamin_d`, `vitamin_b12`, `zinc`, `magnesium`,
    /// `iodine`, `epa`, `dha`, `energy_kcal` or `protein`.
    pub key: String,
    /// The amount per dose, at least 0.
    pub value: f64,
    /// The parameter's unit, or for a mass-based parameter g, mg or µg as on the label (EPA
    /// `180` `mg` is stored as 0.18 g). Convert IU first: vitamin D IU / 40 = µg.
    pub unit: String,
    /// Required for `vitamin_a` (`retinol` for retinol or retinyl esters, `beta_carotene`),
    /// `folate` (`folic_acid`, `methylfolate`), and `niacin_equivalent` or `niacin`
    /// (`nicotinic_acid`, `nicotinamide`); refused for other keys.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub form: Option<String>,
}

/// One dose's amount of a compound the Food database has no parameter for.
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CompoundRequest {
    /// The compound's name, e.g. `Creatine monohydrate`; never a parameter such as vitamin D.
    pub name: String,
    /// The amount per dose, at least 0.
    pub value: f64,
    /// g, mg or µg.
    pub unit: String,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AddSupplementRequest {
    /// The Supplement's name, 1-100 characters, unique in any case, e.g. "Vitamin D3 25 µg".
    pub name: String,
    /// What one dose is: tablet, capsule, scoop or ml.
    pub dose_unit: DoseUnit,
    /// The label's amounts per dose of Food database parameters, each key (and form) once.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amounts: Option<Vec<AmountRequest>>,
    /// The label's amounts per dose of other compounds, each once.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compounds: Option<Vec<CompoundRequest>>,
    /// Unique ID for this Supplement, 1-128 characters. Reuse the identical arguments and ID
    /// after an uncertain response.
    pub request_id: String,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateSupplementRequest {
    /// The Supplement's `id` from `supplements` or `add_supplement`.
    #[serde(skip_serializing)]
    pub supplement_id: String,
    /// A new name, unique in any case. Omit to preserve; cannot be null.
    #[serde(default, skip_serializing_if = "Patch::missing")]
    pub name: Patch<String>,
    /// A new dose unit. Omit to preserve; cannot be null.
    #[serde(default, skip_serializing_if = "Patch::missing")]
    pub dose_unit: Patch<DoseUnit>,
    /// The complete replacement list of per-dose parameter amounts. Omit to preserve; cannot be
    /// null.
    #[serde(default, skip_serializing_if = "Patch::missing")]
    pub amounts: Patch<Vec<AmountRequest>>,
    /// The complete replacement list of per-dose compounds. Omit to preserve; cannot be null.
    #[serde(default, skip_serializing_if = "Patch::missing")]
    pub compounds: Patch<Vec<CompoundRequest>>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SupplementRequest {
    /// The Supplement's `id` from `supplements` or `add_supplement`.
    pub supplement_id: String,
}
