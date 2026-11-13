mod common;

use common::{API_KEY, connect, start_api, start_mcp, wishlist_call};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

#[tokio::test]
#[expect(
    clippy::too_many_lines,
    reason = "one real MCP workflow owns pass-through, clarification, receipt replay and owner-targeted removal"
)]
async fn manual_watchlist_tools_forward_owner_targeted_plans_and_api_receipts_unchanged() {
    let api = start_api().await;
    let http = reqwest::Client::new();
    let mut owners = Vec::new();
    for name in ["Member A", "Member B"] {
        let owner: Value = http
            .post(format!("{api}/api/household-users"))
            .bearer_auth(API_KEY)
            .json(&json!({"name":name}))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        owners.push(owner);
    }
    let mcp = start_mcp(&api, API_KEY).await;
    assert!(connect(&mcp, "wrong-key").await.is_err());
    let client = connect(&mcp, API_KEY).await.unwrap();
    assert_eq!(
        wishlist_call(&client, "household_users", json!({}))
            .await
            .structured_content,
        Some(json!(owners))
    );
    let payload = json!({"kind":"series","title":"A Quiet Orbit","request_id":"mcp-watch-1"});
    let clarification = wishlist_call(&client, "add_watchlist_entry", payload.clone()).await;
    let direct: Value = http
        .post(format!("{api}/api/watchlist/entries"))
        .bearer_auth(API_KEY)
        .json(&payload)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(clarification.is_error, Some(true));
    assert_eq!(direct["code"], "watchlist_destination_required");
    assert_eq!(clarification.structured_content, Some(direct));
    let mut payload = payload;
    payload["owner_id"] = owners[0]["id"].clone();
    let added = wishlist_call(&client, "add_watchlist_entry", payload.clone()).await;
    assert_eq!(added.is_error, Some(false));
    let addition = added.structured_content.unwrap();
    assert_eq!(addition["entry"]["status"], "planned");
    assert_eq!(addition["entry"]["kind"], "series");
    let replayed = wishlist_call(&client, "add_watchlist_entry", payload.clone()).await;
    assert_eq!(
        replayed.structured_content,
        Some(json!({"entry":addition["entry"],"replayed":true}))
    );
    let mut changed = payload.clone();
    changed["owner_id"] = owners[1]["id"].clone();
    assert_eq!(
        wishlist_call(&client, "add_watchlist_entry", changed)
            .await
            .is_error,
        Some(true)
    );
    let listed = wishlist_call(&client, "watchlist_entries", json!({})).await;
    let direct: Value = http
        .get(format!("{api}/api/watchlist/entries"))
        .bearer_auth(API_KEY)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(listed.structured_content, Some(direct));
    let target = json!({"owner_id":owners[0]["id"],"entry_id":addition["entry"]["id"]});
    assert_eq!(
        wishlist_call(&client, "watchlist_entry", target.clone())
            .await
            .structured_content,
        Some(addition["entry"].clone())
    );
    let wrong = json!({"owner_id":owners[1]["id"],"entry_id":addition["entry"]["id"]});
    assert_eq!(
        wishlist_call(&client, "delete_watchlist_entry", wrong)
            .await
            .is_error,
        Some(true)
    );
    assert_eq!(
        wishlist_call(&client, "delete_watchlist_entry", target.clone())
            .await
            .structured_content,
        Some(json!({"deleted":true}))
    );
    assert_eq!(
        wishlist_call(&client, "watchlist_entry", target.clone())
            .await
            .is_error,
        Some(true)
    );
    assert_eq!(
        wishlist_call(&client, "delete_watchlist_entry", target)
            .await
            .is_error,
        Some(true)
    );
    assert_eq!(
        wishlist_call(&client, "add_watchlist_entry", payload)
            .await
            .is_error,
        Some(true)
    );
    assert_eq!(
        wishlist_call(&client, "watchlist_entries", json!({}))
            .await
            .structured_content,
        Some(json!([]))
    );
    client.cancel().await.unwrap();
}

#[tokio::test]
#[expect(
    clippy::too_many_lines,
    reason = "one real MCP workflow protects partial corrections, filtering and pass-through errors"
)]
async fn watchlist_corrections_and_filters_forward_to_real_axum_with_omission_and_null_preserved() {
    let api = start_api().await;
    let http = reqwest::Client::new();
    let owner: Value = http
        .post(format!("{api}/api/household-users"))
        .bearer_auth(API_KEY)
        .json(&json!({"name":"Member A"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let client = connect(&start_mcp(&api, API_KEY).await, API_KEY)
        .await
        .unwrap();
    let added = wishlist_call(&client, "add_watchlist_entry", json!({"owner_id":owner["id"],"kind":"series","title":"A Quiet Orbit","request_id":"correct-watch"})).await.structured_content.unwrap();
    let target = json!({"owner_id":owner["id"],"entry_id":added["entry"]["id"]});
    let path = format!(
        "{api}/api/household-users/{}/watchlist/entries/{}",
        owner["id"].as_str().unwrap(),
        added["entry"]["id"].as_str().unwrap()
    );
    for patch in [
        json!({"status":"watching"}),
        json!({"status":"completed","date_added":"2024-02-28","completion_date":"2024-02-29"}),
        json!({"date_added":"2024-02-27"}),
        json!({"status":"completed"}),
        json!({"status":"planned","completion_date":null}),
    ] {
        let mut args = target.clone();
        args.as_object_mut()
            .unwrap()
            .extend(patch.as_object().unwrap().clone());
        let result = wishlist_call(&client, "update_watchlist_entry", args).await;
        assert_eq!(result.is_error, Some(false), "{result:?}");
        let stored: Value = http
            .get(&path)
            .bearer_auth(API_KEY)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(result.structured_content, Some(stored.clone()));
        assert_eq!(stored["kind"], "series");
        if patch.get("completion_date").is_none() && patch["status"] != "watching" {
            assert_eq!(stored["completion_date"], "2024-02-29");
        }
    }
    let filtered = wishlist_call(
        &client,
        "watchlist_entries",
        json!({"owner_id":owner["id"],"status":"planned"}),
    )
    .await;
    let direct: Value = http
        .get(format!(
            "{api}/api/watchlist/entries?owner_id={}&status=planned",
            owner["id"].as_str().unwrap()
        ))
        .bearer_auth(API_KEY)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(filtered.structured_content, Some(direct));
    assert_eq!(
        wishlist_call(&client, "watchlist_entries", json!({"status":"completed"}))
            .await
            .structured_content,
        Some(json!([]))
    );
    // Owner IDs with query punctuation must remain one encoded value, never additional filters.
    assert_eq!(
        wishlist_call(
            &client,
            "watchlist_entries",
            json!({"owner_id":"missing&status=planned"})
        )
        .await
        .structured_content,
        Some(json!([]))
    );
    for patch in [
        json!({"completion_date":"2024-02-29"}),
        json!({"date_added":null}),
        json!({"status":"completed","completion_date":null}),
        json!({"date_added":"2024-02-30"}),
    ] {
        let mut args = target.clone();
        args.as_object_mut()
            .unwrap()
            .extend(patch.as_object().unwrap().clone());
        let forwarded = wishlist_call(&client, "update_watchlist_entry", args).await;
        let direct: Value = http
            .patch(&path)
            .bearer_auth(API_KEY)
            .json(&patch)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(forwarded.is_error, Some(true));
        assert_eq!(forwarded.structured_content, Some(direct));
    }
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn watchlist_write_auth_and_transport_failures_remain_tool_errors() {
    let api = start_api().await;
    for (api_url, key, expected_status) in [
        (api.as_str(), "wrong-key", json!(401)),
        ("http://127.0.0.1:1", API_KEY, Value::Null),
    ] {
        let client = connect(&start_mcp(api_url, key).await, API_KEY)
            .await
            .unwrap();
        for (tool, args) in [
            (
                "add_watchlist_entry",
                json!({"owner_id":"synthetic-owner","kind":"movie","title":"A Quiet Orbit","request_id":"failed-watch"}),
            ),
            (
                "update_watchlist_entry",
                json!({"owner_id":"synthetic-owner","entry_id":"synthetic-entry","status":"completed"}),
            ),
        ] {
            let result = wishlist_call(&client, tool, args).await;
            assert_eq!(result.is_error, Some(true));
            assert_eq!(
                result.structured_content.unwrap()["status"],
                expected_status
            );
        }
        client.cancel().await.unwrap();
    }
}
