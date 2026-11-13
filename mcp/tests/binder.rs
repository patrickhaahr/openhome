mod common;

use common::{API_KEY, connect, start_api, start_mcp, wishlist_call};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

#[tokio::test]
async fn search_cards_forwards_the_catalogue_search_unchanged() {
    let api = start_api().await;
    let client = connect(&start_mcp(&api, API_KEY).await, API_KEY)
        .await
        .unwrap();
    let instructions = client
        .peer_info()
        .unwrap()
        .instructions
        .clone()
        .unwrap_or_default();
    assert!(
        instructions.contains("call search_cards")
            && instructions.contains("card_data_unavailable"),
        "{instructions}"
    );
    let direct = |query: &'static str| {
        let url = format!("{api}/api/binder/cards?card_code={query}");
        async move {
            reqwest::Client::new()
                .get(url)
                .bearer_auth(API_KEY)
                .send()
                .await
                .unwrap()
                .json::<Value>()
                .await
                .unwrap()
        }
    };

    let found = wishlist_call(&client, "search_cards", json!({"card_code": "OP99-001"})).await;
    assert_eq!(found.is_error, Some(false), "{found:?}");
    let found = found.structured_content.unwrap();
    assert_eq!(
        (
            &found["card_code"],
            &found["cards"][0]["id"],
            &found["cards"][1]["id"],
            &found["cards"][0]["card_value"]
        ),
        (
            &json!("OP99-001"),
            &json!("OP99-001"),
            &json!("OP99-001_p1"),
            &json!({"usd": 2.25, "observed_on": "2026-11-12"})
        ),
        "{found}"
    );
    assert_eq!(found, direct("OP99-001").await);

    let unlisted = wishlist_call(&client, "search_cards", json!({"card_code": "OP99-002"})).await;
    assert_eq!(unlisted.is_error, Some(false), "{unlisted:?}");
    assert_eq!(
        unlisted.structured_content.unwrap(),
        json!({"card_code": "OP99-002", "cards": []})
    );

    let malformed = wishlist_call(&client, "search_cards", json!({"card_code": "OP99"})).await;
    assert_eq!(malformed.is_error, Some(true));
    let malformed = malformed.structured_content.unwrap();
    assert_eq!(malformed["status"], 400);
    assert_eq!(malformed, direct("OP99").await);
    client.cancel().await.unwrap();
}
