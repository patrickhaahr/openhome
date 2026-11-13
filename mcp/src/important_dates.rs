use super::patch::Patch;
use crate::OpenHomeTools;
use rmcp::schemars;
use rmcp::{handler::server::wrapper::Parameters, model::CallToolResult, tool, tool_router};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum DateKind {
    /// Counts up from the date: years, months and days elapsed.
    Since,
    /// Counts down to the date's next anniversary.
    Yearly,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ImportantDatesRequest {
    /// Europe/Copenhagen day YYYY-MM-DD to compute figures for; omit for today.
    pub on: Option<String>,
    /// Omit for all dates; true for only those shown on the idle screen, false for the rest.
    pub show_when_idle: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AddImportantDateRequest {
    /// Generic title, 1-200 characters, e.g. "Together" or "Alex's birthday".
    pub title: String,
    /// The date itself, a real YYYY-MM-DD calendar date.
    pub date: String,
    /// `since` counts up from the date; `yearly` counts down to its next anniversary.
    pub kind: DateKind,
    /// Stable Household User IDs from `household_users` that the date concerns; [] for none. Required: never infer tags.
    pub person_ids: Vec<String>,
    /// Whether the shared idle screen may show it. Defaults to false; anyone in the room can see the idle screen.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub show_when_idle: Option<bool>,
    /// Unique intended addition ID, 1-128 characters. Reuse identical arguments and ID after an uncertain response.
    pub request_id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ImportantDateRequest {
    /// Stable Important Date ID from `important_dates` or `add_important_date`.
    pub date_id: String,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateImportantDateRequest {
    /// Stable Important Date ID from `important_dates` or `add_important_date`.
    #[serde(skip_serializing)]
    pub date_id: String,
    /// New title, 1-200 characters. Omit to preserve; cannot be null.
    #[serde(default, skip_serializing_if = "Patch::missing")]
    pub title: Patch<String>,
    /// Corrected YYYY-MM-DD date. Omit to preserve; cannot be null.
    #[serde(default, skip_serializing_if = "Patch::missing")]
    pub date: Patch<String>,
    /// since or yearly. Omit to preserve; cannot be null.
    #[serde(default, skip_serializing_if = "Patch::missing")]
    pub kind: Patch<DateKind>,
    /// The complete replacement set of tagged Household User IDs; [] clears every tag. Omit to preserve; cannot be null.
    #[serde(default, skip_serializing_if = "Patch::missing")]
    pub person_ids: Patch<Vec<String>>,
    /// Omit to preserve; cannot be null.
    #[serde(default, skip_serializing_if = "Patch::missing")]
    pub show_when_idle: Patch<bool>,
}

#[tool_router(router = important_dates_router, vis = "pub(crate)")]
impl OpenHomeTools {
    #[tool(
        description = "List household-shared Important Dates with figures for one Europe/Copenhagen day (today unless `on` YYYY-MM-DD is given). Each date has a stable id, title, date, kind (since/yearly), tagged person_ids (0..n Household User IDs), show_when_idle, `elapsed` (years, months, days and total_days since the date; null while it is ahead) and `next` (the next anniversary's date, days_until, 0 on the day, and which anniversary in years). Since dates come first, then yearly dates by next occurrence. Each date also carries milestone: anniversary with years, day_count with days, yearly with years, or null. Since milestones celebrate anniversaries (preferred on collisions), 100, 500 and every 1,000 days; yearly milestones occur on their day. Use show_when_idle=true for idle-safe milestones. Optional show_when_idle filter.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn important_dates(
        &self,
        Parameters(request): Parameters<ImportantDatesRequest>,
    ) -> CallToolResult {
        let mut query = Vec::new();
        if let Some(on) = request.on {
            query.push(("on".into(), on));
        }
        if let Some(idle) = request.show_when_idle {
            query.push(("show_when_idle".into(), idle.to_string()));
        }
        self.api
            .request(
                reqwest::Method::GET,
                &["api", "important-dates"],
                None,
                &query,
            )
            .await
    }

    #[tool(
        description = "Read one Important Date by its stable date_id, with today's figures and milestone (anniversary with years, day_count with days, yearly with years, or null). A removed date is a tool error. Read after an uncertain update or deletion before deciding to retry.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn important_date(
        &self,
        Parameters(request): Parameters<ImportantDateRequest>,
    ) -> CallToolResult {
        self.api
            .get(&["api", "important-dates", &request.date_id])
            .await
    }

    #[tool(
        description = "Add a household-shared Important Date: a generic title, a YYYY-MM-DD date, kind since (count up) or yearly (count down to the next anniversary), and person_ids: the explicit Household User IDs from household_users it concerns, [] for none. Never infer tags. show_when_idle defaults to false. Generate a unique request_id per intention; reuse identical arguments and ID after an uncertain response. Accepted retries return replayed=true; changed arguments conflict.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn add_important_date(
        &self,
        Parameters(request): Parameters<AddImportantDateRequest>,
    ) -> CallToolResult {
        self.api
            .post(&["api", "important-dates"], json!(request))
            .await
    }

    #[tool(
        description = "Update supplied fields of one Important Date by date_id. Omitted fields are preserved; none can be null. person_ids replaces every tag with explicit Household User IDs ([] clears them); never infer tags. After an uncertain response, read the date before retrying.",
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn update_important_date(
        &self,
        Parameters(request): Parameters<UpdateImportantDateRequest>,
    ) -> CallToolResult {
        self.api
            .request(
                reqwest::Method::PATCH,
                &["api", "important-dates", &request.date_id],
                Some(json!(request)),
                &[],
            )
            .await
    }

    #[tool(
        description = "Permanently delete one Important Date by date_id. Returns deleted=true only after Axum confirms removal; a missing date is a tool error. Read after an uncertain response. Old addition retries cannot recreate it.",
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn delete_important_date(
        &self,
        Parameters(request): Parameters<ImportantDateRequest>,
    ) -> CallToolResult {
        self.api
            .request(
                reqwest::Method::DELETE,
                &["api", "important-dates", &request.date_id],
                None,
                &[],
            )
            .await
    }
}
