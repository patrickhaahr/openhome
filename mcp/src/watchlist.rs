use super::patch::Patch;
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

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum WatchStatus {
    Planned,
    Watching,
    Completed,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WatchlistEntriesRequest {
    /// Omit for all owners, or use a stable ID from `household_users`.
    pub owner_id: Option<String>,
    /// Omit for all statuses, or filter planned/watching/completed.
    pub status: Option<WatchStatus>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateWatchlistEntryRequest {
    /// Explicit Household User ID; must own `entry_id`.
    #[serde(skip_serializing)]
    pub owner_id: String,
    /// Stable entry ID from `watchlist_entries` or `add_watchlist_entry`.
    #[serde(skip_serializing)]
    pub entry_id: String,
    /// Omit to preserve. Completing records today unless a date is supplied; leaving completed clears it. Cannot be null.
    #[serde(default, skip_serializing_if = "Patch::missing")]
    pub status: Patch<WatchStatus>,
    /// Explicit YYYY-MM-DD correction; omission preserves the original added date. Cannot be null.
    #[serde(default, skip_serializing_if = "Patch::missing")]
    pub date_added: Patch<String>,
    /// Explicit YYYY-MM-DD correction for completed entries, no earlier than `date_added`. Omit to preserve; null only valid outside completed.
    #[serde(default, skip_serializing_if = "Patch::missing")]
    pub completion_date: Patch<String>,
}
