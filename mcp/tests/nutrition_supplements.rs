mod common;

use common::{API_KEY, connect, start_api, start_mcp, wishlist_call};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

#[tokio::test]
#[expect(
    clippy::too_many_lines,
    reason = "one contract follows a Supplement from its label through a Meal to a split day"
)]
async fn supplement_tools_forward_labels_doses_meals_and_the_day_split_unchanged() {
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
        instructions.contains("Protein powder")
            && instructions.contains("are Products, not Supplements")
            && instructions.contains("supplement_doses"),
        "{instructions}"
    );
    let http = reqwest::Client::new();
    let direct = |method: reqwest::Method, path: &str, body: Option<Value>| {
        let mut request = http
            .request(method, format!("{api}{path}"))
            .bearer_auth(API_KEY);
        if let Some(body) = body {
            request = request.json(&body);
        }
        async move { request.send().await.unwrap().json::<Value>().await.unwrap() }
    };

    let vitamin_d = json!({
        "name": "Vitamin D3",
        "dose_unit": "capsule",
        "amounts": [{"key": "vitamin_d", "value": 25, "unit": "µg"}],
        "request_id": "mcp-vitamin-d",
    });
    let added = wishlist_call(&client, "add_supplement", vitamin_d.clone()).await;
    assert_eq!(added.is_error, Some(false), "{added:?}");
    let vitamin_d_supplement = added.structured_content.unwrap()["supplement"].clone();
    assert_eq!(
        wishlist_call(&client, "add_supplement", vitamin_d)
            .await
            .structured_content,
        Some(json!({"supplement": vitamin_d_supplement, "replayed": true}))
    );
    let creatine = wishlist_call(
        &client,
        "add_supplement",
        json!({
            "name": "Creatine",
            "dose_unit": "scoop",
            "compounds": [{"name": "Creatine monohydrate", "value": 3, "unit": "g"}],
            "request_id": "mcp-creatine",
        }),
    )
    .await
    .structured_content
    .unwrap()["supplement"]
        .clone();
    let listed = wishlist_call(&client, "supplements", json!({})).await;
    let expected = direct(reqwest::Method::GET, "/api/nutrition/supplements", None).await;
    assert_eq!(
        expected,
        json!({"supplements": [creatine, vitamin_d_supplement]})
    );
    assert_eq!(listed.structured_content, Some(expected));
    let formless = json!({
        "name": "Folate",
        "dose_unit": "tablet",
        "amounts": [{"key": "folate", "value": 400, "unit": "µg"}],
        "request_id": "mcp-folate",
    });
    let refused = wishlist_call(&client, "add_supplement", formless.clone()).await;
    assert_eq!(refused.is_error, Some(true));
    assert_eq!(
        refused.structured_content,
        Some(
            direct(
                reqwest::Method::POST,
                "/api/nutrition/supplements",
                Some(formless)
            )
            .await
        )
    );

    let stack = wishlist_call(
        &client,
        "save_meal",
        json!({
            "name": "Morning stack",
            "supplement_doses": [
                {"supplement_id": vitamin_d_supplement["id"], "doses": 1},
                {"supplement_id": creatine["id"], "doses": 1},
            ],
            "request_id": "mcp-stack",
        }),
    )
    .await;
    assert_eq!(stack.is_error, Some(false), "{stack:?}");
    let meal = stack.structured_content.unwrap()["meal"].clone();
    assert_eq!(meal["supplement_doses"][1]["name"], "Creatine");
    let logged = wishlist_call(
        &client,
        "log_meal",
        json!({
            "eaten_at": "2026-10-29T07:30:00+01:00",
            "portions": [{"food_id": 1693, "grams": 150}],
            "supplement_doses": [{"supplement_id": vitamin_d_supplement["id"], "doses": 2}],
            "request_id": "mcp-breakfast",
        }),
    )
    .await;
    assert_eq!(logged.is_error, Some(false), "{logged:?}");
    let by_name = wishlist_call(
        &client,
        "log_meal",
        json!({
            "eaten_at": "2026-10-29T21:00:00+01:00",
            "meal": "morning stack",
            "overrides": [{"supplement_id": creatine["id"], "doses": 2}],
            "request_id": "mcp-evening",
        }),
    )
    .await;
    assert_eq!(by_name.is_error, Some(false), "{by_name:?}");
    let meal_log = by_name.structured_content.unwrap()["meal_log"].clone();
    assert_eq!(
        meal_log["supplement_doses"][1]["compounds"],
        json!([{"name": "Creatine monohydrate", "value": 6.0, "unit": "g"}])
    );

    let day = wishlist_call(&client, "nutrition_day", json!({"day": "2026-10-29"}))
        .await
        .structured_content
        .unwrap();
    assert_eq!(
        day,
        direct(reqwest::Method::GET, "/api/nutrition/days/2026-10-29", None).await
    );
    let vitamin_d_total = day["nutrients"]
        .as_array()
        .unwrap()
        .iter()
        .find(|nutrient| nutrient["key"] == "vitamin_d")
        .unwrap();
    assert_eq!(
        (&vitamin_d_total["supplements"], &vitamin_d_total["food"]),
        (&json!(75.0), &Value::Null)
    );
    assert_eq!(
        day["compounds"],
        json!([{"name": "Creatine monohydrate", "value": 6.0, "unit": "g"}])
    );

    let creatine_id = creatine["id"].as_str().unwrap();
    let in_meal = wishlist_call(
        &client,
        "delete_supplement",
        json!({"supplement_id": creatine_id}),
    )
    .await;
    assert_eq!(in_meal.is_error, Some(true));
    assert_eq!(in_meal.structured_content.unwrap()["status"], 409);
    let updated = wishlist_call(
        &client,
        "update_meal",
        json!({
            "meal_id": meal["id"],
            "supplement_doses": [{"supplement_id": vitamin_d_supplement["id"], "doses": 1}],
        }),
    )
    .await;
    assert_eq!(updated.is_error, Some(false), "{updated:?}");
    let renamed = wishlist_call(
        &client,
        "update_supplement",
        json!({"supplement_id": creatine_id, "name": "Creatine 5 g", "compounds": [
            {"name": "Creatine monohydrate", "value": 5, "unit": "g"},
        ]}),
    )
    .await;
    assert_eq!(renamed.is_error, Some(false), "{renamed:?}");
    assert_eq!(renamed.structured_content.unwrap()["name"], "Creatine 5 g");
    let null_name = wishlist_call(
        &client,
        "update_supplement",
        json!({"supplement_id": creatine_id, "name": null}),
    )
    .await;
    assert_eq!(null_name.is_error, Some(true));
    let target = json!({"supplement_id": creatine_id});
    assert_eq!(
        wishlist_call(&client, "delete_supplement", target.clone())
            .await
            .structured_content,
        Some(json!({"deleted": true}))
    );
    assert_eq!(
        wishlist_call(&client, "delete_supplement", target)
            .await
            .is_error,
        Some(true)
    );
    // Past days keep the doses they logged.
    assert_eq!(
        wishlist_call(&client, "nutrition_day", json!({"day": "2026-10-29"}))
            .await
            .structured_content,
        Some(day)
    );
    client.cancel().await.unwrap();
}
