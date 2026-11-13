use super::patch::Patch;
use crate::OpenHomeTools;
use rmcp::schemars;
use rmcp::{handler::server::wrapper::Parameters, model::CallToolResult, tool, tool_router};
use serde::{Deserialize, Serialize};
use serde_json::json;

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

#[tool_router(router = watchlist_router, vis = "pub(crate)")]
impl OpenHomeTools {
    #[tool(
        description = "List household-visible Watchlist Entries with optional owner_id and planned/watching/completed status filters. Returns stable IDs, media kind, title, personal status, date added and completion date. Use household_users to discover owners with empty lists. Does not infer viewing history.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn watchlist_entries(
        &self,
        Parameters(request): Parameters<WatchlistEntriesRequest>,
    ) -> CallToolResult {
        let mut query = Vec::new();
        if let Some(owner) = request.owner_id {
            query.push(("owner_id".into(), owner));
        }
        if let Some(status) = request.status {
            // Serialize only the typed enum; query encoding belongs to the HTTP client.
            let value = match status {
                WatchStatus::Planned => "planned",
                WatchStatus::Watching => "watching",
                WatchStatus::Completed => "completed",
            };
            query.push(("status".into(), value.into()));
        }
        self.api
            .request(
                reqwest::Method::GET,
                &["api", "watchlist", "entries"],
                None,
                &query,
            )
            .await
    }

    #[tool(
        description = "Add a manual movie or whole-series plan for an explicit owner_id discovered with household_users. Missing owner returns one clarification question with choices and writes nothing. Generate a unique request_id per intention; reuse identical arguments and ID after an uncertain response. Accepted retries return replayed=true; changed arguments conflict. New entries start planned. No provider search or completion is inferred.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn add_watchlist_entry(
        &self,
        Parameters(request): Parameters<AddWatchlistEntryRequest>,
    ) -> CallToolResult {
        self.api
            .post(&["api", "watchlist", "entries"], json!(request))
            .await
    }

    #[tool(
        description = "Read one household-visible Watchlist Entry by explicit stable owner_id and entry_id. A mismatched or removed destination is a tool error. Read after an uncertain removal before deciding to retry.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn watchlist_entry(
        &self,
        Parameters(request): Parameters<WatchlistEntryRequest>,
    ) -> CallToolResult {
        self.api
            .get(&[
                "api",
                "household-users",
                &request.owner_id,
                "watchlist",
                "entries",
                &request.entry_id,
            ])
            .await
    }

    #[tool(
        description = "Correct a Watchlist Entry's personal status/dates by explicit owner_id and entry_id. Omitted fields are preserved. Completing records today in Europe/Copenhagen unless completion_date is supplied; leaving completed clears it. Dates must be real YYYY-MM-DD and completion cannot precede addition. Whole series only; never infer completion from providers. After an uncertain response, read the entry before retrying.",
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn update_watchlist_entry(
        &self,
        Parameters(request): Parameters<UpdateWatchlistEntryRequest>,
    ) -> CallToolResult {
        self.api
            .request(
                reqwest::Method::PATCH,
                &[
                    "api",
                    "household-users",
                    &request.owner_id,
                    "watchlist",
                    "entries",
                    &request.entry_id,
                ],
                Some(json!(request)),
                &[],
            )
            .await
    }

    #[tool(
        description = "Permanently remove one Watchlist Entry by explicit owner_id and entry_id. Returns deleted=true only after Axum confirms removal; missing or mismatched destinations are tool errors. Read after an uncertain response. Old addition retries cannot recreate the removed entry.",
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn delete_watchlist_entry(
        &self,
        Parameters(request): Parameters<WatchlistEntryRequest>,
    ) -> CallToolResult {
        self.api
            .request(
                reqwest::Method::DELETE,
                &[
                    "api",
                    "household-users",
                    &request.owner_id,
                    "watchlist",
                    "entries",
                    &request.entry_id,
                ],
                None,
                &[],
            )
            .await
    }
}
