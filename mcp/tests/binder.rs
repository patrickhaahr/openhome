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

#[tokio::test]
async fn binder_entry_tools_forward_additions_and_lists_unchanged() {
    let api = start_api().await;
    let http = reqwest::Client::new();
    let direct_get = |query: String| {
        let http = http.clone();
        let url = format!("{api}/api/binder/entries{query}");
        async move {
            http.get(url)
                .bearer_auth(API_KEY)
                .send()
                .await
                .unwrap()
                .json::<Value>()
                .await
                .unwrap()
        }
    };
    let owner: Value = http
        .post(format!("{api}/api/household-users"))
        .bearer_auth(API_KEY)
        .json(&json!({"name": "Member A"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let client = connect(&start_mcp(&api, API_KEY).await, API_KEY)
        .await
        .unwrap();
    wishlist_call(&client, "search_cards", json!({"card_code": "OP99-001"})).await;
    let request = json!({"card_id": "OP99-001_p1", "quantity": 2, "language": "ja", "request_id": "mcp-binder-1"});
    let clarification = wishlist_call(&client, "add_binder_entry", request.clone()).await;
    assert_eq!(clarification.is_error, Some(true));
    let clarification = clarification.structured_content.unwrap();
    assert_eq!(clarification["code"], "binder_destination_required");
    assert_eq!(clarification["choices"], json!([owner]));

    let mut request = request;
    request["owner_id"] = owner["id"].clone();
    let added = wishlist_call(&client, "add_binder_entry", request.clone()).await;
    assert_eq!(added.is_error, Some(false), "{added:?}");
    let added = added.structured_content.unwrap();
    assert_eq!(
        (
            &added["replayed"],
            &added["entry"]["card"]["id"],
            &added["entry"]["language"],
            &added["entry"]["quantity"]
        ),
        (
            &json!(false),
            &json!("OP99-001_p1"),
            &json!("ja"),
            &json!(2)
        ),
        "{added}"
    );

    let replayed = wishlist_call(&client, "add_binder_entry", request).await;
    assert_eq!(replayed.is_error, Some(false));
    let replayed = replayed.structured_content.unwrap();
    assert_eq!(replayed["replayed"], true);
    assert_eq!(replayed["entry"], added["entry"]);

    let unlisted = wishlist_call(
        &client,
        "add_binder_entry",
        json!({"owner_id": owner["id"], "card_id": "OP99-002", "request_id": "mcp-binder-2"}),
    )
    .await;
    assert_eq!(unlisted.is_error, Some(true));
    let unlisted = unlisted.structured_content.unwrap();
    assert_eq!(
        (&unlisted["status"], &unlisted["code"]),
        (&json!(404), &json!("card_not_listed"))
    );

    let owner_id = owner["id"].as_str().unwrap();
    for (arguments, query) in [
        (json!({}), String::new()),
        (
            json!({"owner_id": owner_id}),
            format!("?owner_id={owner_id}"),
        ),
        (json!({"owner_id": "nobody"}), "?owner_id=nobody".into()),
    ] {
        let listed = wishlist_call(&client, "binder_entries", arguments).await;
        assert_eq!(listed.is_error, Some(false), "{listed:?}");
        assert_eq!(listed.structured_content.unwrap(), direct_get(query).await);
    }
    client.cancel().await.unwrap();
}

type Client = rmcp::service::RunningService<rmcp::RoleClient, rmcp::model::ClientConfig>;

async fn two_entries(api: &str) -> (Client, String, Vec<Value>) {
    let http = reqwest::Client::new();
    let owner_a: Value = http
        .post(format!("{api}/api/household-users"))
        .bearer_auth(API_KEY)
        .json(&json!({"name": "Member A"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let owner_a = owner_a["id"].as_str().unwrap().to_owned();
    let client = connect(&start_mcp(api, API_KEY).await, API_KEY)
        .await
        .unwrap();
    let mut entries = vec![];
    for (language, request_id) in [("en", "mcp-edit-1"), ("ja", "mcp-edit-2")] {
        let added = wishlist_call(
            &client,
            "add_binder_entry",
            json!({"owner_id": owner_a, "card_id": "OP99-001", "language": language,
                   "quantity": 2, "note": "sleeved", "request_id": request_id}),
        )
        .await;
        assert_eq!(added.is_error, Some(false), "{added:?}");
        entries.push(added.structured_content.unwrap()["entry"]["id"].clone());
    }
    (client, owner_a, entries)
}

#[tokio::test]
async fn update_binder_entry_forwards_the_request_unchanged() {
    let api = start_api().await;
    let http = reqwest::Client::new();
    let (client, owner_a, entries) = two_entries(&api).await;
    let uri = |entry: &Value| {
        format!(
            "{api}/api/household-users/{owner_a}/binder/entries/{}",
            entry.as_str().unwrap()
        )
    };
    let direct_patch = |entry: Value, body: Value| {
        let http = http.clone();
        let url = uri(&entry);
        async move {
            http.patch(url)
                .bearer_auth(API_KEY)
                .json(&body)
                .send()
                .await
                .unwrap()
                .json::<Value>()
                .await
                .unwrap()
        }
    };

    for patch in [
        json!({"quantity": 5}),
        json!({"note": null}),
        json!({"note": "binder 2", "quantity": 1}),
    ] {
        let mut arguments = patch.clone();
        arguments["owner_id"] = json!(owner_a);
        arguments["entry_id"] = entries[0].clone();
        let updated = wishlist_call(&client, "update_binder_entry", arguments).await;
        assert_eq!(updated.is_error, Some(false), "{updated:?}");
        assert_eq!(
            updated.structured_content.unwrap(),
            direct_patch(entries[0].clone(), patch).await
        );
    }
    let read_back = wishlist_call(&client, "binder_entries", json!({"owner_id": owner_a})).await;
    let read_back = read_back.structured_content.unwrap();
    assert_eq!(
        (&read_back[0]["quantity"], &read_back[0]["note"]),
        (&json!(1), &json!("binder 2"))
    );

    let collision = json!({"owner_id": owner_a, "entry_id": entries[0], "language": "ja"});
    let conflict = wishlist_call(&client, "update_binder_entry", collision).await;
    assert_eq!(conflict.is_error, Some(true));
    let conflict = conflict.structured_content.unwrap();
    assert_eq!(conflict["status"], 409);
    assert_eq!(
        conflict,
        direct_patch(entries[0].clone(), json!({"language": "ja"})).await
    );
    let invalid = wishlist_call(
        &client,
        "update_binder_entry",
        json!({"owner_id": owner_a, "entry_id": entries[0], "quantity": 0}),
    )
    .await;
    assert_eq!(invalid.is_error, Some(true));
    assert_eq!(invalid.structured_content.unwrap()["status"], 400);
    let wrong_owner = wishlist_call(
        &client,
        "update_binder_entry",
        json!({"owner_id": "nobody", "entry_id": entries[0], "quantity": 2}),
    )
    .await;
    assert_eq!(wrong_owner.is_error, Some(true));
    assert_eq!(wrong_owner.structured_content.unwrap()["status"], 404);
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn delete_binder_entry_forwards_the_request_unchanged() {
    let api = start_api().await;
    let (client, owner_a, entries) = two_entries(&api).await;
    let target = json!({"owner_id": owner_a, "entry_id": entries[0]});
    let wrong = json!({"owner_id": "nobody", "entry_id": entries[0]});
    assert_eq!(
        wishlist_call(&client, "delete_binder_entry", wrong)
            .await
            .is_error,
        Some(true)
    );
    assert_eq!(
        wishlist_call(&client, "delete_binder_entry", target.clone())
            .await
            .structured_content,
        Some(json!({"deleted": true}))
    );
    let repeated = wishlist_call(&client, "delete_binder_entry", target).await;
    assert_eq!(repeated.is_error, Some(true));
    assert_eq!(repeated.structured_content.unwrap()["status"], 404);
    let left = wishlist_call(&client, "binder_entries", json!({"owner_id": owner_a})).await;
    let left = left.structured_content.unwrap();
    assert_eq!(left.as_array().unwrap().len(), 1);
    assert_eq!(left[0]["id"], entries[1]);
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn global_instructions_describe_the_binder_photo_workflow() {
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
        instructions.contains("add_binder_entry")
            && instructions.contains("binder_destination_required")
            && instructions.contains("Never store the user's photo"),
        "{instructions}"
    );
    client.cancel().await.unwrap();
}
