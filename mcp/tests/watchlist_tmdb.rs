mod common;

use common::{
    API_KEY, TMDB_MOVIE_ID, connect, start_api, start_api_with_tmdb, start_mcp, wishlist_call,
};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

async fn owner(api: &str) -> Value {
    reqwest::Client::new()
        .post(format!("{api}/api/household-users"))
        .bearer_auth(API_KEY)
        .json(&json!({"name": "Member A"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
}

#[tokio::test]
async fn a_found_tmdb_id_adds_an_enriched_entry_that_reads_back_the_same() {
    let api = start_api_with_tmdb().await;
    let owner = owner(&api).await;
    let client = connect(&start_mcp(&api, API_KEY).await, API_KEY)
        .await
        .unwrap();

    let found = wishlist_call(
        &client,
        "search_watchlist_titles",
        json!({"kind": "movie", "query": "Orbit"}),
    )
    .await;
    assert_eq!(found.is_error, Some(false), "{found:?}");
    let found = found.structured_content.unwrap();
    assert_eq!(found["items"][0]["tmdb_id"], TMDB_MOVIE_ID);
    assert_eq!(found["items"][0]["title"], "A Quiet Orbit");

    let added = wishlist_call(
        &client,
        "add_watchlist_entry",
        json!({
            "owner_id": owner["id"],
            "kind": "movie",
            "title": "orbit",
            "tmdb_id": found["items"][0]["tmdb_id"],
            "request_id": "mcp-tmdb-1",
        }),
    )
    .await;
    assert_eq!(added.is_error, Some(false), "{added:?}");
    let entry = added.structured_content.unwrap()["entry"].clone();
    assert_eq!(entry["tmdb_id"], TMDB_MOVIE_ID);
    assert_eq!(entry["title"], "A Quiet Orbit");
    assert_eq!(
        entry["overview"],
        "An entirely fictional journey among the stars."
    );
    assert_eq!(entry["poster_path"], "/synthetic.jpg");
    assert_eq!(entry["release_date"], "2020-02-29");

    let read_back = wishlist_call(
        &client,
        "watchlist_entry",
        json!({"owner_id": owner["id"], "entry_id": entry["id"]}),
    )
    .await;
    assert_eq!(read_back.structured_content, Some(entry));
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn tmdb_failures_and_unknown_ids_are_tool_errors_and_manual_adds_stay_available() {
    let configured = start_api_with_tmdb().await;
    let owner_a = owner(&configured).await;
    let client = connect(&start_mcp(&configured, API_KEY).await, API_KEY)
        .await
        .unwrap();
    let unknown = wishlist_call(
        &client,
        "add_watchlist_entry",
        json!({
            "owner_id": owner_a["id"], "kind": "movie", "title": "Unknown",
            "tmdb_id": 999, "request_id": "mcp-tmdb-unknown",
        }),
    )
    .await;
    assert_eq!(unknown.is_error, Some(true));
    assert_eq!(
        unknown.structured_content.unwrap()["code"],
        "tmdb_unavailable"
    );
    client.cancel().await.unwrap();

    let unconfigured = start_api().await;
    let owner_b = owner(&unconfigured).await;
    let client = connect(&start_mcp(&unconfigured, API_KEY).await, API_KEY)
        .await
        .unwrap();
    let search = wishlist_call(
        &client,
        "search_watchlist_titles",
        json!({"kind": "series", "query": "Lighthouse"}),
    )
    .await;
    assert_eq!(search.is_error, Some(true));
    assert_eq!(
        search.structured_content.unwrap()["code"],
        "tmdb_unavailable"
    );
    let manual = wishlist_call(
        &client,
        "add_watchlist_entry",
        json!({
            "owner_id": owner_b["id"], "kind": "series", "title": "Offline story",
            "request_id": "mcp-manual-1",
        }),
    )
    .await;
    assert_eq!(manual.is_error, Some(false));
    let entry = manual.structured_content.unwrap()["entry"].clone();
    assert_eq!(entry["title"], "Offline story");
    assert_eq!(entry["tmdb_id"], Value::Null);
    client.cancel().await.unwrap();
}
