use super::patch::Patch;
use rmcp::schemars;
use serde::{Deserialize, Serialize};

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
