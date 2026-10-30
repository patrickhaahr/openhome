//! Food database tool arguments, forwarded unchanged to Axum's nutrition routes.

use rmcp::schemars;
use serde::Deserialize;

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
