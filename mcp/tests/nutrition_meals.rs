mod common;

use common::{
    API_KEY, SYNTHETIC_BARCODE, connect, food_database, start_api, start_api_with_foods, start_mcp,
    wishlist_call,
};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

#[tokio::test]
#[expect(
    clippy::too_many_lines,
    reason = "one contract follows a Meal from saving through logging by name to deletion"
)]
async fn meal_tools_save_list_log_by_name_rename_and_delete_through_the_api_unchanged() {
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
        instructions.contains("Call save_meal only when the user names a meal")
            && instructions.contains("offer once to save it")
            && instructions.contains("meal set to its name")
            && instructions.contains("without analysing any photo"),
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
    let linked = wishlist_call(
        &client,
        "link_product",
        json!({"barcode": SYNTHETIC_BARCODE, "food_id": 1693, "request_id": "mcp-meal-link"}),
    )
    .await;
    assert_eq!(linked.is_error, Some(false), "{linked:?}");

    let payload = json!({
        "name": "Protein oats",
        "portions": [
            {"food_id": 1693, "grams": 150},
            {"barcode": SYNTHETIC_BARCODE, "grams": 100},
        ],
        "request_id": "mcp-save-1",
    });
    let saved = wishlist_call(&client, "save_meal", payload.clone()).await;
    assert_eq!(saved.is_error, Some(false), "{saved:?}");
    let saved = saved.structured_content.unwrap();
    assert_eq!(saved["replayed"], false);
    let meal = saved["meal"].clone();
    assert_eq!(
        meal["portions"][1],
        json!({
            "food_id": null, "barcode": SYNTHETIC_BARCODE, "grams": 100.0,
            "name_da": "Skyr, 0.2 % fedt", "name_en": "Skyr, 0.2 % fat",
            "product": {"barcode": SYNTHETIC_BARCODE, "name": "Vanilla Skyr", "brand": "Synthetic Dairy"},
        })
    );
    assert_eq!(
        wishlist_call(&client, "save_meal", payload.clone())
            .await
            .structured_content,
        Some(json!({"meal": meal, "replayed": true}))
    );
    let listed = wishlist_call(&client, "meals", json!({})).await;
    let expected = direct(reqwest::Method::GET, "/api/nutrition/meals", None).await;
    assert_eq!(expected, json!({"meals": [meal]}));
    assert_eq!(listed.structured_content, Some(expected));
    let mut shouted = payload.clone();
    shouted["name"] = json!("PROTEIN OATS");
    shouted["request_id"] = json!("mcp-save-2");
    let taken = wishlist_call(&client, "save_meal", shouted).await;
    assert_eq!(taken.is_error, Some(true));
    assert_eq!(taken.structured_content.unwrap()["status"], 409);

    let by_name = json!({
        "eaten_at": "2026-10-29T08:15:00+01:00",
        "meal": "protein OATS",
        "overrides": [{"food_id": 1693, "grams": 200}],
        "request_id": "mcp-meal-by-name",
    });
    let logged = wishlist_call(&client, "log_meal", by_name.clone()).await;
    assert_eq!(logged.is_error, Some(false), "{logged:?}");
    let meal_log = logged.structured_content.unwrap()["meal_log"].clone();
    assert_eq!(meal_log["meal_name"], "Protein oats");
    assert_eq!(
        (
            &meal_log["portions"][0]["grams"],
            &meal_log["portions"][0]["nutrients"]["energy_kcal"],
            &meal_log["portions"][1]["product"]["barcode"],
            &meal_log["portions"][1]["nutrients"]["energy_kcal"],
        ),
        (
            &json!(200.0),
            &json!({"value": 124.0, "unit": "kcal", "source": "food"}),
            &json!(SYNTHETIC_BARCODE),
            &json!({"value": 76.0, "unit": "kcal", "source": "label"}),
        )
    );

    let id = meal["id"].as_str().unwrap();
    let updated = wishlist_call(
        &client,
        "update_meal",
        json!({"meal_id": id, "name": "Skyr bowl", "portions": [{"food_id": 1693, "grams": 100}]}),
    )
    .await;
    assert_eq!(updated.is_error, Some(false), "{updated:?}");
    let updated = updated.structured_content.unwrap();
    assert_eq!(
        (
            &updated["name"],
            updated["portions"].as_array().unwrap().len()
        ),
        (&json!("Skyr bowl"), 1)
    );
    let null_portions = wishlist_call(
        &client,
        "update_meal",
        json!({"meal_id": id, "portions": null}),
    )
    .await;
    assert_eq!(null_portions.is_error, Some(true));
    // The Meal Log keeps what it logged.
    let day = wishlist_call(&client, "nutrition_day", json!({"day": "2026-10-29"}))
        .await
        .structured_content
        .unwrap();
    assert_eq!(day["meal_logs"], json!([meal_log]));

    let target = json!({"meal_id": id});
    assert_eq!(
        wishlist_call(&client, "delete_meal", target.clone())
            .await
            .structured_content,
        Some(json!({"deleted": true}))
    );
    for (tool, arguments) in [
        ("delete_meal", target),
        ("update_meal", json!({"meal_id": id, "name": "Late"})),
        ("save_meal", payload),
    ] {
        assert_eq!(
            wishlist_call(&client, tool, arguments).await.is_error,
            Some(true),
            "{tool}"
        );
    }
    let mut unknown = by_name;
    unknown["meal"] = json!("Lasagne");
    unknown["request_id"] = json!("mcp-meal-unknown");
    let result = wishlist_call(&client, "log_meal", unknown.clone()).await;
    assert_eq!(result.is_error, Some(true));
    assert_eq!(
        result.structured_content,
        Some(
            direct(
                reqwest::Method::POST,
                "/api/nutrition/meal-logs",
                Some(unknown)
            )
            .await
        )
    );

    let importing = start_api_with_foods(food_database(false).await).await;
    let client = connect(&start_mcp(&importing, API_KEY).await, API_KEY)
        .await
        .unwrap();
    let result = wishlist_call(
        &client,
        "save_meal",
        json!({
            "name": "Skyr",
            "portions": [{"food_id": 1693, "grams": 150}],
            "request_id": "mcp-save-3",
        }),
    )
    .await;
    assert_eq!(result.is_error, Some(true));
    let error = result.structured_content.unwrap();
    assert_eq!(
        (&error["code"], &error["status"]),
        (&json!("food_database_unavailable"), &json!(503)),
        "{error}"
    );
    let meals = wishlist_call(&client, "meals", json!({})).await;
    assert_eq!(meals.structured_content, Some(json!({"meals": []})));
    client.cancel().await.unwrap();
}
