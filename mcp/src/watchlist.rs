use rmcp::schemars;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum MediaKind {
    Movie,
    Series,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AddWatchlistEntryRequest {
    /// Stable Household User ID from `household_users`. Omission returns clarification, never a guessed owner.
    pub owner_id: Option<String>,
    /// Movie or whole series; no episode or season tracking.
    pub kind: MediaKind,
    /// Manual title, 1-500 characters. No provider metadata is fetched.
    pub title: String,
    /// Unique intended addition ID, 1-128 characters. Reuse identical arguments and ID after an uncertain response.
    pub request_id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WatchlistEntryRequest {
    /// Explicit stable Household User ID; must own `entry_id`.
    pub owner_id: String,
    /// Stable Watchlist Entry ID from `watchlist_entries` or `add_watchlist_entry`.
    pub entry_id: String,
}
