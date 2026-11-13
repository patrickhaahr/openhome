use rmcp::{
    handler::server::wrapper::Parameters, model::CallToolResult, schemars, tool, tool_router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{OpenHomeTools, patch::Patch};

const SEARCH_CARDS: &str = "Look up a One Piece Card Game card by its Card Code (for example OP01-001) and get every variant of it: the normal print and each parallel art. Each Card has a stable id (OP01-001, OP01-001_p1), its name, set, rarity, colour, type, life, cost, power, counter amount, attribute, types and text, a card_value (its TCGplayer market price in usd, eur and dkk, converted at the ECB rate of the price's own day or the latest earlier one, with observed_on, rate_date and a stale flag for a price observed more than 30 days before today in Copenhagen) or null with value_unavailable saying why (no_catalogue_price, no_exchange_rates), and an image_path for the stored AVIF image. The first lookup of a code asks the OPTCG catalogue and stores the Cards; later lookups come from OpenHome. A code the catalogue does not list returns cards as an empty list; a malformed code is an error, and card_data_unavailable means the catalogue could not be reached, so try again later. There is no name search.";

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SearchCardsRequest {
    /// The Card Code printed on the card, such as OP01-001: a set prefix, a hyphen and three digits. Malformed codes are rejected.
    pub card_code: String,
}

const CARD_PRICES: &str = "Read one Card's Price History by its Card id from search_cards or binder_entries: the TCGplayer market prices OpenHome observed since the Card was first stored (a search or a Binder add stores it), oldest first, each with observed_on, usd and its eur and dkk converted at the ECB rate of that observation date (or the latest earlier day, named in rate_date; null when no rate exists yet). Optional from and to bound observed_on as inclusive YYYY-MM-DD days; omit both for the whole history. A price whose observation date has not changed adds no point, and nothing is backfilled, so the points are sparse and gaps between them mean no new observation, not a flat price; never interpolate. An unknown id is a 404 tool error and a card with no priced observation returns an empty prices list. Answer trends in DKK unless the user asks for EUR or USD.";

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CardPricesRequest {
    /// The Card's `id` from `search_cards`, such as OP01-001 or its parallel OP01-001_p1.
    pub card_id: String,
    /// Earliest observation day to include, YYYY-MM-DD. Omit for no lower bound.
    pub from: Option<String>,
    /// Latest observation day to include, YYYY-MM-DD. Omit for no upper bound.
    pub to: Option<String>,
}

const ADD_BINDER_ENTRY: &str = "Add a One Piece Card Game card to a Household User's Binder, by the Card id from search_cards (OP01-001, OP01-001_p1) and an explicit owner_id discovered with household_users. Searching the Card Code first is the recommended workflow. An id nobody searched is resolved through the Card Code it contains (OP01-001_p1, EB03_OP09-034_p1) and added when that code lists it; an id containing no Card Code must come from search_cards. A rejected addition writes nothing. Axum stores the Card itself and never trusts card details from the client; a card the catalogue does not list is a 404 card_not_listed tool error and writes nothing, and card_data_unavailable (503) means the catalogue could not be reached. Quantity (a whole number of at least 1, default 1) is how many copies this addition adds: adding an owner's existing Card in the same language raises its quantity and returns the updated entry. Language is en (default) or ja, and English and Japanese copies are separate entries; a ja entry has no card_value and value_unavailable japanese_printing, because catalogue prices are for English printings. A missing owner returns binder_destination_required with one question and choices and writes nothing. Generate a unique request_id per intended addition and reuse identical arguments and ID after an uncertain response: an accepted retry returns the original result with replayed=true and never adds twice, changed arguments conflict, and a removed entry cannot be recreated by an old request_id.";

const BINDER_ENTRIES: &str = "List Binder Entries, household-visible, with an optional owner_id filter (omit for every owner; use household_users to discover owners). Returns entries and binder_values. Each entry has its stable id, owner_id, language (en or ja), quantity, optional note, the Europe/Copenhagen date_added and its Card as search_cards returns it, including card_value in usd, eur and dkk, except that a ja entry has no card_value and value_unavailable japanese_printing because catalogue prices are for English printings. Entries come back grouped by owner, each Binder in Card Code order: set prefix, set number, card number, with a normal print before its parallels. binder_values has one item per Household User in scope, including an empty Binder worth zero: owner_id and that Binder Value, the sum of quantity times card_value over priced entries in usd, eur and dkk (stale prices count), unpriced_entries (entries with no card_value, left out of the sums) and oldest_observed_on (the oldest price date among the priced entries, null when none). Answer values in DKK unless the user asks for EUR or USD.";

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

const UPDATE_BINDER_ENTRY: &str = "Correct one Binder Entry's quantity, language or note by explicit owner_id and entry_id from binder_entries or add_binder_entry. Omitted fields are preserved; note null clears the note; quantity is a whole number of at least 1 and replaces the stored quantity (it does not add to it); quantity and language cannot be null. Changing language to one the owner already holds for that Card returns a 409 conflict and changes nothing: raise that entry's quantity and remove this one instead. A ja entry has no card_value (value_unavailable japanese_printing). The result is the stored entry. A wrong owner or entry is a 404 tool error. After an uncertain response, read binder_entries before retrying.";

const DELETE_BINDER_ENTRY: &str = "Permanently remove one Binder Entry by explicit owner_id and entry_id. The Card and its Price History stay. Returns deleted=true only after Axum confirms removal; a wrong owner or an entry already removed is a 404 tool error, so a repeated delete reports not found. Old addition retries cannot recreate the removed entry. Read binder_entries after an uncertain response.";

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BinderEntryRequest {
    /// Explicit stable Household User ID; must own `entry_id`.
    pub owner_id: String,
    /// Stable Binder Entry ID from `binder_entries` or `add_binder_entry`.
    pub entry_id: String,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateBinderEntryRequest {
    /// Explicit Household User ID; must own `entry_id`.
    #[serde(skip_serializing)]
    pub owner_id: String,
    /// Stable Binder Entry ID from `binder_entries` or `add_binder_entry`.
    #[serde(skip_serializing)]
    pub entry_id: String,
    /// Omit to preserve. The new copy count, a whole number of at least 1. Cannot be null.
    #[serde(default, skip_serializing_if = "Patch::missing")]
    pub quantity: Patch<i64>,
    /// Omit to preserve. `en` or `ja`; a collision with the owner's other entry for the Card is a 409. Cannot be null.
    #[serde(default, skip_serializing_if = "Patch::missing")]
    pub language: Patch<Language>,
    /// Omit to preserve, 1-500 characters to replace, null to clear.
    #[serde(default, skip_serializing_if = "Patch::missing")]
    pub note: Patch<String>,
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
        description = CARD_PRICES,
        annotations(title = "Card Prices", read_only_hint = true, open_world_hint = false)
    )]
    async fn card_prices(
        &self,
        Parameters(CardPricesRequest { card_id, from, to }): Parameters<CardPricesRequest>,
    ) -> CallToolResult {
        let query: Vec<_> = [("from", from), ("to", to)]
            .into_iter()
            .filter_map(|(name, day)| Some((name.into(), day?)))
            .collect();
        self.api
            .request(
                reqwest::Method::GET,
                &["api", "binder", "cards", &card_id, "prices"],
                None,
                &query,
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

    #[tool(
        description = UPDATE_BINDER_ENTRY,
        annotations(
            title = "Update Binder Entry",
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn update_binder_entry(
        &self,
        Parameters(request): Parameters<UpdateBinderEntryRequest>,
    ) -> CallToolResult {
        self.api
            .request(
                reqwest::Method::PATCH,
                &[
                    "api",
                    "household-users",
                    &request.owner_id,
                    "binder",
                    "entries",
                    &request.entry_id,
                ],
                Some(json!(request)),
                &[],
            )
            .await
    }

    #[tool(
        description = DELETE_BINDER_ENTRY,
        annotations(
            title = "Delete Binder Entry",
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn delete_binder_entry(
        &self,
        Parameters(request): Parameters<BinderEntryRequest>,
    ) -> CallToolResult {
        self.api
            .request(
                reqwest::Method::DELETE,
                &[
                    "api",
                    "household-users",
                    &request.owner_id,
                    "binder",
                    "entries",
                    &request.entry_id,
                ],
                None,
                &[],
            )
            .await
    }
}
