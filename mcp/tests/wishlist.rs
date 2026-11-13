mod common;

use common::{API_KEY, connect, start_api, start_mcp, wishlist_call};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

#[tokio::test]
async fn wishlist_mutations_and_clarification_pass_through_the_authenticated_api() {
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
    let owners = wishlist_call(&client, "household_users", json!({})).await;
    assert_eq!(owners.structured_content, Some(json!([owner])));
    let created = wishlist_call(
        &client,
        "create_wishlist",
        json!({"owner_id":owner["id"],"name":"Ideas"}),
    )
    .await;
    assert_eq!(created.is_error, Some(false));
    let list = created.structured_content.unwrap();
    assert_eq!(list["owner_id"], owner["id"]);
    let lists = wishlist_call(&client, "wishlists", json!({})).await;
    assert_eq!(lists.structured_content, Some(json!([list])));
    let payload = json!({"title":"A warm jacket","request_id":"mcp-wish-1"});
    let ambiguous = wishlist_call(&client, "add_wishlist_item", payload.clone()).await;
    assert_eq!(ambiguous.is_error, Some(true));
    let direct: Value = http
        .post(format!("{api}/api/wishlist/items"))
        .bearer_auth(API_KEY)
        .json(&payload)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(ambiguous.structured_content, Some(direct.clone()));
    assert_eq!(direct["code"], "wishlist_destination_required");
    let mut payload = payload;
    payload["list_id"] = list["id"].clone();
    let added = wishlist_call(&client, "add_wishlist_item", payload.clone()).await;
    assert_eq!(added.is_error, Some(false));
    let addition = added.structured_content.unwrap();
    assert_eq!(addition["item"]["title"], "A warm jacket");
    let replayed = wishlist_call(&client, "add_wishlist_item", payload.clone()).await;
    assert_eq!(replayed.is_error, Some(false));
    assert_eq!(
        replayed.structured_content,
        Some(json!({"item":addition["item"],"replayed":true}))
    );
    let items = wishlist_call(&client, "wishlist_items", json!({"list_id":list["id"]})).await;
    let direct: Value = http
        .get(format!(
            "{api}/api/wishlists/{}/items",
            list["id"].as_str().unwrap()
        ))
        .bearer_auth(API_KEY)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(items.is_error, Some(false));
    assert_eq!(items.structured_content, Some(direct.clone()));
    assert_eq!(direct, json!([addition["item"]]));
    payload["title"] = json!("Another wish");
    let conflict = wishlist_call(&client, "add_wishlist_item", payload.clone()).await;
    let direct: Value = http
        .post(format!("{api}/api/wishlist/items"))
        .bearer_auth(API_KEY)
        .json(&payload)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(conflict.is_error, Some(true));
    assert_eq!(conflict.structured_content, Some(direct));
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn wishlist_write_auth_and_transport_failures_remain_tool_errors() {
    let api = start_api().await;
    for (api_url, key, expected_status) in [
        (api.as_str(), "wrong-key", json!(401)),
        ("http://127.0.0.1:1", API_KEY, Value::Null),
    ] {
        let client = connect(&start_mcp(api_url, key).await, API_KEY)
            .await
            .unwrap();
        let result = wishlist_call(
            &client,
            "add_wishlist_item",
            json!({"title":"A warm jacket","request_id":"failed-1"}),
        )
        .await;
        assert_eq!(result.is_error, Some(true));
        assert_eq!(
            result.structured_content.unwrap()["status"],
            expected_status
        );
        client.cancel().await.unwrap();
    }
}

#[tokio::test]
#[expect(
    clippy::too_many_lines,
    reason = "one real transport lifecycle protects partial writes, retries and deletion"
)]
async fn rich_item_management_uses_real_transport_with_partial_updates_and_deletion() {
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
    let list = wishlist_call(
        &client,
        "create_wishlist",
        json!({"owner_id":owner["id"],"name":"Outdoors"}),
    )
    .await
    .structured_content
    .unwrap();
    let payload = json!({"owner_id":owner["id"],"list_id":list["id"],"title":"A jacket","request_id":"rich-mcp-1",
        "url":"https://example.com/jacket","image_url":"https://example.com/jacket.png","notes":"For winter","price":"DKK 199.95","quantity":2,"priority":"high"});
    let blocked = wishlist_call(
        &client,
        "wishlist_url_metadata",
        json!({"owner_id":owner["id"],"list_id":list["id"],"url":"http://127.0.0.1/product"}),
    )
    .await;
    assert_eq!(blocked.is_error, Some(true));
    assert_eq!(blocked.structured_content.as_ref().unwrap()["status"], 400);
    assert_eq!(
        wishlist_call(&client, "wishlist_items", json!({"list_id":list["id"]}))
            .await
            .structured_content,
        Some(json!([]))
    );
    let wrong = wishlist_call(
        &client,
        "wishlist_url_metadata",
        json!({"owner_id":"wrong-owner","list_id":list["id"],"url":"https://example.com/jacket"}),
    )
    .await;
    assert_eq!(wrong.is_error, Some(true));
    assert_eq!(wrong.structured_content.unwrap()["status"], 409);
    let added = wishlist_call(&client, "add_wishlist_item", payload.clone()).await;
    assert_eq!(added.is_error, Some(false));
    let item = added.structured_content.unwrap()["item"].clone();
    assert_eq!(item["quantity"], 2);
    assert_eq!(item["priority"], "high");
    let target = json!({"owner_id":owner["id"],"list_id":list["id"],"item_id":item["id"]});
    assert_eq!(
        wishlist_call(&client, "wishlist_item", target.clone())
            .await
            .structured_content,
        Some(item.clone())
    );
    let mut edit = target.clone();
    edit["notes"] = Value::Null;
    edit["quantity"] = json!(3);
    let result = wishlist_call(&client, "update_wishlist_item", edit.clone()).await;
    assert_eq!(result.is_error, Some(false));
    let edited = result.structured_content.unwrap();
    assert_eq!(edited["notes"], Value::Null);
    assert_eq!(edited["quantity"], 3);
    assert_eq!(edited["title"], item["title"]);
    assert_eq!(edited["url"], item["url"]);
    assert_eq!(edited["price"], item["price"]);
    let direct: Value = http
        .get(format!(
            "{api}/api/household-users/{}/wishlists/{}/items/{}",
            owner["id"].as_str().unwrap(),
            list["id"].as_str().unwrap(),
            item["id"].as_str().unwrap()
        ))
        .bearer_auth(API_KEY)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(edited, direct);
    assert_eq!(
        wishlist_call(&client, "update_wishlist_item", edit.clone())
            .await
            .structured_content,
        Some(edited.clone())
    );
    edit["quantity"] = json!(0);
    let invalid = wishlist_call(&client, "update_wishlist_item", edit).await;
    assert_eq!(invalid.is_error, Some(true));
    assert_eq!(invalid.structured_content.unwrap()["status"], 400);
    assert_eq!(
        wishlist_call(&client, "wishlist_item", target.clone())
            .await
            .structured_content,
        Some(edited.clone())
    );
    let replay = wishlist_call(&client, "add_wishlist_item", payload.clone()).await;
    assert_eq!(
        replay.structured_content,
        Some(json!({"item":edited,"replayed":true}))
    );
    let mut wrong = target.clone();
    wrong["owner_id"] = json!("other-owner");
    for tool in [
        "wishlist_item",
        "update_wishlist_item",
        "delete_wishlist_item",
    ] {
        let result = wishlist_call(&client, tool, wrong.clone()).await;
        assert_eq!(result.is_error, Some(true));
        assert_eq!(result.structured_content.unwrap()["status"], 404);
    }
    let removed = wishlist_call(&client, "delete_wishlist_item", target.clone()).await;
    assert_eq!(removed.is_error, Some(false));
    assert_eq!(removed.structured_content, Some(json!({"deleted":true})));
    assert_eq!(
        wishlist_call(&client, "delete_wishlist_item", target)
            .await
            .is_error,
        Some(true)
    );
    assert_eq!(
        wishlist_call(&client, "add_wishlist_item", payload)
            .await
            .is_error,
        Some(true)
    );
    assert_eq!(
        wishlist_call(&client, "wishlist_items", json!({"list_id":list["id"]}))
            .await
            .structured_content,
        Some(json!([]))
    );
    client.cancel().await.unwrap();
}
