use rmcp::{
    handler::server::wrapper::Parameters, model::CallToolResult, schemars, tool, tool_router,
};
use serde::Deserialize;

use crate::OpenHomeTools;

const SEARCH_CARDS: &str = "Look up a One Piece Card Game card by its Card Code (for example OP01-001) and get every variant of it: the normal print and each parallel art. Each Card has a stable id (OP01-001, OP01-001_p1), its name, set, rarity, colour, type, life, cost, power, counter amount, attribute, types and text, a card_value with its USD market price and the date the catalogue observed it (null when unpriced), and an image_path for the stored AVIF image. The first lookup of a code asks the OPTCG catalogue and stores the Cards; later lookups come from OpenHome. A code the catalogue does not list returns cards as an empty list; a malformed code is an error, and card_data_unavailable means the catalogue could not be reached, so try again later. There is no name search.";

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SearchCardsRequest {
    /// The Card Code printed on the card, such as OP01-001: a set prefix, a hyphen and three digits. Malformed codes are rejected.
    pub card_code: String,
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
}
