mod common;

use common::{API_KEY, connect, start_api, start_mcp, wishlist_call};
use pretty_assertions::assert_eq;
use rmcp::model::CallToolRequestParams;
use serde_json::{Value, json};

#[tokio::test]
#[expect(
    clippy::too_many_lines,
    reason = "one real MCP workflow owns explicit tags, receipt replay, partial updates and removal"
)]
async fn important_date_tools_forward_explicit_tags_receipts_and_figures_unchanged() {
    let api = start_api().await;
    let http = reqwest::Client::new();
    let mut people = Vec::new();
    for name in ["Member A", "Member B"] {
        let person: Value = http
            .post(format!("{api}/api/household-users"))
            .bearer_auth(API_KEY)
            .json(&json!({"name":name}))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        people.push(person["id"].clone());
    }
    let client = connect(&start_mcp(&api, API_KEY).await, API_KEY)
        .await
        .unwrap();
    // Tags are required: a date with no person_ids at all is rejected before Axum.
    let untagged = client
        .call_tool(
            CallToolRequestParams::new("add_important_date").with_arguments(
                json!({"title":"Together","date":"2024-02-14","kind":"since","request_id":"mcp-date-1"})
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
        )
        .await;
    assert!(untagged.is_err() || untagged.unwrap().is_error == Some(true));
    let payload = json!({"title":"Together","date":"2024-02-14","kind":"since","person_ids":people,"request_id":"mcp-date-1"});
    let added = wishlist_call(&client, "add_important_date", payload.clone()).await;
    assert_eq!(added.is_error, Some(false));
    let addition = added.structured_content.unwrap();
    assert_eq!(addition["replayed"], false);
    assert_eq!(addition["important_date"]["show_when_idle"], false);
    assert_eq!(
        wishlist_call(&client, "add_important_date", payload.clone())
            .await
            .structured_content,
        Some(json!({"important_date":addition["important_date"],"replayed":true}))
    );
    let mut changed = payload.clone();
    changed["person_ids"] = json!([]);
    assert_eq!(
        wishlist_call(&client, "add_important_date", changed)
            .await
            .is_error,
        Some(true)
    );
    let friend = wishlist_call(
        &client,
        "add_important_date",
        json!({"title":"A friend's birthday","date":"2000-02-29","kind":"yearly","person_ids":[],"show_when_idle":true,"request_id":"mcp-date-2"}),
    )
    .await;
    assert_eq!(
        friend.structured_content.unwrap()["important_date"]["person_ids"],
        json!([])
    );

    let listed = wishlist_call(&client, "important_dates", json!({"on":"2026-05-26"})).await;
    let direct: Value = http
        .get(format!("{api}/api/important-dates?on=2026-05-26"))
        .bearer_auth(API_KEY)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(listed.structured_content, Some(direct.clone()));
    assert_eq!(
        direct["dates"][0]["elapsed"],
        json!({"years":2,"months":3,"days":12,"total_days":832})
    );
    assert_eq!(direct["dates"][1]["next"]["date"], "2027-02-28");
    let anniversary = wishlist_call(&client, "important_dates", json!({"on":"2026-02-14"})).await;
    assert_eq!(
        anniversary.structured_content.unwrap()["dates"][0]["milestone"],
        json!({"kind":"anniversary","years":2})
    );
    let idle = wishlist_call(&client, "important_dates", json!({"show_when_idle":true})).await;
    assert_eq!(
        idle.structured_content.unwrap()["dates"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        wishlist_call(&client, "important_dates", json!({"on":"2026-02-30"}))
            .await
            .is_error,
        Some(true)
    );

    let target = json!({"date_id":addition["important_date"]["id"]});
    let mut update = target.clone();
    update["person_ids"] = json!([people[1]]);
    update["show_when_idle"] = json!(true);
    let updated = wishlist_call(&client, "update_important_date", update)
        .await
        .structured_content
        .unwrap();
    assert_eq!(updated["person_ids"], json!([people[1]]));
    assert_eq!(updated["title"], "Together");
    assert_eq!(updated["show_when_idle"], true);
    let mut null_title = target.clone();
    null_title["title"] = Value::Null;
    assert_eq!(
        wishlist_call(&client, "update_important_date", null_title)
            .await
            .is_error,
        Some(true)
    );
    assert_eq!(
        wishlist_call(&client, "important_date", target.clone())
            .await
            .structured_content,
        Some(updated)
    );
    assert_eq!(
        wishlist_call(&client, "delete_important_date", target.clone())
            .await
            .structured_content,
        Some(json!({"deleted":true}))
    );
    for tool in ["important_date", "delete_important_date"] {
        assert_eq!(
            wishlist_call(&client, tool, target.clone()).await.is_error,
            Some(true)
        );
    }
    assert_eq!(
        wishlist_call(&client, "add_important_date", payload)
            .await
            .is_error,
        Some(true)
    );
    client.cancel().await.unwrap();
}
