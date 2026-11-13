use rmcp::{
    handler::server::wrapper::Parameters, model::CallToolResult, schemars, tool, tool_router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::OpenHomeTools;

const SEARCH_CARDS: &str = "Look up a One Piece Card Game card by its Card Code (for example OP01-001) and get every variant of it: the normal print and each parallel art. Each Card has a stable id (OP01-001, OP01-001_p1), its name, set, rarity, colour, type, life, cost, power, counter amount, attribute, types and text, a card_value with its USD market price and the date the catalogue observed it (null when unpriced), and an image_path for the stored AVIF image. The first lookup of a code asks the OPTCG catalogue and stores the Cards; later lookups come from OpenHome. A code the catalogue does not list returns cards as an empty list; a malformed code is an error, and card_data_unavailable means the catalogue could not be reached, so try again later. There is no name search.";

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SearchCardsRequest {
    /// The Card Code printed on the card, such as OP01-001: a set prefix, a hyphen and three digits. Malformed codes are rejected.
    pub card_code: String,
}

const ADD_BINDER_ENTRY: &str = "Add a One Piece Card Game card to a Household User's Binder, by the Card id from search_cards (OP01-001, OP01-001_p1) and an explicit owner_id discovered with household_users. Searching the Card Code first is the recommended workflow, but an id nobody searched is still added when the catalogue lists it. Axum stores the Card itself and never trusts card details from the client; a card the catalogue does not list is a 404 card_not_listed tool error and writes nothing, and card_data_unavailable (503) means the catalogue could not be reached. Quantity (a whole number of at least 1, default 1) is how many copies this addition adds: adding an owner's existing Card in the same language raises its quantity and returns the updated entry. Language is en (default) or ja, and English and Japanese copies are separate entries; a ja entry has no card_value and no inventory_price_usd, because catalogue prices are for English printings. A missing owner returns binder_destination_required with one question and choices and writes nothing. Generate a unique request_id per intended addition and reuse identical arguments and ID after an uncertain response: an accepted retry returns the original result with replayed=true and never adds twice, changed arguments conflict, and a removed entry cannot be recreated by an old request_id.";

const BINDER_ENTRIES: &str = "List Binder Entries, household-visible, with an optional owner_id filter (omit for every owner; use household_users to discover owners). Each entry has its stable id, owner_id, language (en or ja), quantity, optional note, the Europe/Copenhagen date_added and its Card as search_cards returns it, including card_value in USD, except that a ja entry has no card_value or inventory_price_usd because catalogue prices are for English printings. Entries come back grouped by owner, each Binder in Card Code order: set prefix, set number, card number, with a normal print before its parallels. Binder Value and other currencies are not included yet.";

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    En,
    Ja,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AddBinderEntryRequest {
    /// Stable Household User ID from `household_users`. Omission returns clarification, never a guessed owner.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner_id: Option<String>,
    /// The Card's `id` from `search_cards`, such as OP01-001 or its parallel OP01-001_p1. Never a Card Code guessed from the photo.
    pub card_id: String,
    /// Copies this addition adds, at least 1. Omit for 1.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quantity: Option<i64>,
    /// `en` unless the card is visibly Japanese; omit for `en`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<Language>,
    /// Optional note of 1-500 characters. On an existing entry a supplied note replaces the stored one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// Unique intended addition ID, 1-128 characters. Reuse identical arguments and ID after an uncertain response.
    pub request_id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BinderEntriesRequest {
    /// Omit for all owners, or use a stable ID from `household_users`.
    pub owner_id: Option<String>,
}

#[tool_router(router = binder_router, vis = "pub(crate)")]
impl OpenHomeTools {
    #[tool(
        description = SEARCH_CARDS,
        annotations(title = "Search Cards", read_only_hint = true, open_world_hint = true)
    )]
    async fn search_cards(
        &self,
        Parameters(SearchCardsRequest { card_code }): Parameters<SearchCardsRequest>,
    ) -> CallToolResult {
        self.api
            .request(
                reqwest::Method::GET,
                &["api", "binder", "cards"],
                None,
                &[("card_code".into(), card_code)],
            )
            .await
    }

    #[tool(
        description = ADD_BINDER_ENTRY,
        annotations(
            title = "Add Binder Entry",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = true
        )
    )]
    async fn add_binder_entry(
        &self,
        Parameters(request): Parameters<AddBinderEntryRequest>,
    ) -> CallToolResult {
        self.api
            .post(&["api", "binder", "entries"], json!(request))
            .await
    }

    #[tool(
        description = BINDER_ENTRIES,
        annotations(title = "Binder Entries", read_only_hint = true, open_world_hint = false)
    )]
    async fn binder_entries(
        &self,
        Parameters(BinderEntriesRequest { owner_id }): Parameters<BinderEntriesRequest>,
    ) -> CallToolResult {
        let query: Vec<_> = owner_id
            .into_iter()
            .map(|owner| ("owner_id".into(), owner))
            .collect();
        self.api
            .request(
                reqwest::Method::GET,
                &["api", "binder", "entries"],
                None,
                &query,
            )
            .await
    }
}
