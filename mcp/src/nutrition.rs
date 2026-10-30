//! Food database, NOVA Class, Product and Meal Log tool arguments, forwarded unchanged to Axum's nutrition
//! routes.

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
    /// 1-50 Portions: a mixed dish split into its Foods.
    pub portions: Vec<PortionRequest>,
    /// Unique ID for this intended meal, 1-128 characters. Reuse the identical arguments and ID
    /// after an uncertain response.
    pub request_id: String,
    /// Optional photo of the meal: the user's JPEG or PNG file as base64, at most 10 MB before
    /// encoding. Stored without its metadata as AVIF with a thumbnail.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub photo: Option<String>,
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
    /// The complete replacement list of 1-50 Portions, snapshotted afresh from the current Food
    /// database. Omit to preserve; cannot be null.
    #[serde(default, skip_serializing_if = "Patch::missing")]
    pub portions: Patch<Vec<PortionRequest>>,
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
