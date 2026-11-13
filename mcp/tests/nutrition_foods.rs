mod common;

use common::{
    API_KEY, SYNTHETIC_BARCODE, connect, food_database, start_api, start_api_with_foods, start_mcp,
    wishlist_call,
};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

#[tokio::test]
async fn classify_food_forwards_classes_and_api_errors_unchanged() {
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
        instructions.contains("nova_class null")
            && instructions.contains("call classify_food once"),
        "{instructions}"
    );
    let http = reqwest::Client::new();
    let direct = |method: reqwest::Method, path: String, body: Option<Value>| {
        let mut request = http
            .request(method, format!("{api}{path}"))
            .bearer_auth(API_KEY);
        if let Some(body) = body {
            request = request.json(&body);
        }
        async move { request.send().await.unwrap().json::<Value>().await.unwrap() }
    };
    let classify = |food_id: u32, nova_class: u8| {
        direct(
            reqwest::Method::PUT,
            format!("/api/nutrition/foods/{food_id}/nova-class"),
            Some(json!({"nova_class": nova_class})),
        )
    };

    let classified = wishlist_call(
        &client,
        "classify_food",
        json!({"food_id": 1693, "nova_class": 1}),
    )
    .await;

    assert_eq!(classified.is_error, Some(false), "{classified:?}");
    let classified = classified.structured_content.unwrap();
    assert_eq!(
        (&classified["food_id"], &classified["nova_class"]),
        (&json!(1693), &json!(1)),
        "{classified}"
    );
    // Setting the same class again is idempotent and reads the same.
    assert_eq!(classify(1693, 1).await, classified);
    let food = direct(
        reqwest::Method::GET,
        "/api/nutrition/foods/1693".into(),
        None,
    )
    .await;
    assert_eq!(food["nova_class"], 1);

    for (food_id, nova_class) in [(1693, 5), (424_242, 2)] {
        let result = wishlist_call(
            &client,
            "classify_food",
            json!({"food_id": food_id, "nova_class": nova_class}),
        )
        .await;
        assert_eq!(result.is_error, Some(true), "{food_id} {nova_class}");
        assert_eq!(
            result.structured_content,
            Some(classify(food_id, nova_class).await),
            "{food_id} {nova_class}"
        );
    }

    let importing = start_api_with_foods(food_database(false).await).await;
    let client = connect(&start_mcp(&importing, API_KEY).await, API_KEY)
        .await
        .unwrap();
    let result = wishlist_call(
        &client,
        "classify_food",
        json!({"food_id": 1693, "nova_class": 1}),
    )
    .await;
    assert_eq!(result.is_error, Some(true));
    let error = result.structured_content.unwrap();
    assert_eq!(
        (&error["code"], &error["status"]),
        (&json!("food_database_unavailable"), &json!(503)),
        "{error}"
    );
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn food_tools_forward_searches_reads_and_api_errors_unchanged() {
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
        instructions.contains("density (g/ml) are grams")
            && instructions.contains("(1 - waste/100) is the edible weight"),
        "{instructions}"
    );
    let http = reqwest::Client::new();
    let direct = |path: &'static str| {
        let http = http.clone();
        let url = format!("{api}{path}");
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

    let search = wishlist_call(
        &client,
        "search_foods",
        json!({"query": "skyr", "limit": 5}),
    )
    .await;
    assert_eq!(search.is_error, Some(false));
    let expected = direct("/api/nutrition/foods?query=skyr&limit=5").await;
    assert_eq!(expected["foods"][0]["food_id"], 1693, "{expected}");
    assert_eq!(search.structured_content, Some(expected));

    let food = wishlist_call(&client, "food", json!({"food_id": 1693})).await;
    assert_eq!(food.is_error, Some(false));
    let expected = direct("/api/nutrition/foods/1693").await;
    assert_eq!(expected["density"]["value"], 1.03, "{expected}");
    assert_eq!(food.structured_content, Some(expected));

    for (tool, arguments, path) in [
        (
            "food",
            json!({"food_id": 424_242}),
            "/api/nutrition/foods/424242",
        ),
        (
            "search_foods",
            json!({"query": " "}),
            "/api/nutrition/foods?query=%20",
        ),
    ] {
        let result = wishlist_call(&client, tool, arguments).await;
        assert_eq!(result.is_error, Some(true), "{tool}");
        assert_eq!(
            result.structured_content,
            Some(direct(path).await),
            "{tool}"
        );
    }

    let importing = start_api_with_foods(food_database(false).await).await;
    let client = connect(&start_mcp(&importing, API_KEY).await, API_KEY)
        .await
        .unwrap();
    for (tool, arguments) in [
        ("search_foods", json!({"query": "skyr"})),
        ("food", json!({"food_id": 1693})),
    ] {
        let result = wishlist_call(&client, tool, arguments).await;
        assert_eq!(result.is_error, Some(true), "{tool}");
        let error = result.structured_content.unwrap();
        assert_eq!(
            (&error["code"], &error["state"], &error["status"]),
            (
                &json!("food_database_unavailable"),
                &json!("importing"),
                &json!(503)
            ),
            "{tool}: {error}"
        );
    }
    client.cancel().await.unwrap();
}

#[tokio::test]
#[expect(
    clippy::too_many_lines,
    reason = "one contract follows a barcode from preview through linking to a logged Portion"
)]
async fn product_tools_forward_lookups_links_and_product_portions_unchanged() {
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
        instructions.contains("call lookup_product")
            && instructions.contains("call link_product once")
            && instructions.contains("product_not_found"),
        "{instructions}"
    );
    let http = reqwest::Client::new();
    let direct = |path: String| {
        let http = http.clone();
        let url = format!("{api}{path}");
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
    let product_path = format!("/api/nutrition/products/{SYNTHETIC_BARCODE}");

    let preview = wishlist_call(
        &client,
        "lookup_product",
        json!({"barcode": SYNTHETIC_BARCODE}),
    )
    .await;
    assert_eq!(preview.is_error, Some(false), "{preview:?}");
    let preview = preview.structured_content.unwrap();
    assert_eq!(
        (
            &preview["linked"],
            &preview["product"]["brand"],
            &preview["suggested_foods"][0]["food_id"]
        ),
        (&json!(false), &json!("Synthetic Dairy"), &json!(1693)),
        "{preview}"
    );
    assert_eq!(preview, direct(product_path.clone()).await);

    let link = json!({"barcode": SYNTHETIC_BARCODE, "food_id": 1693, "request_id": "mcp-link-1"});
    let linked = wishlist_call(&client, "link_product", link.clone()).await;
    assert_eq!(linked.is_error, Some(false), "{linked:?}");
    let linked = linked.structured_content.unwrap();
    assert_eq!(
        (&linked["replayed"], &linked["product"]["food"]["food_id"]),
        (&json!(false), &json!(1693))
    );
    assert_eq!(
        wishlist_call(&client, "link_product", link)
            .await
            .structured_content
            .unwrap()["replayed"],
        true
    );
    let lookup = wishlist_call(
        &client,
        "lookup_product",
        json!({"barcode": SYNTHETIC_BARCODE}),
    )
    .await
    .structured_content
    .unwrap();
    assert_eq!(
        (&lookup["linked"], &lookup["product"]),
        (&json!(true), &linked["product"])
    );
    assert_eq!(lookup, direct(product_path).await);

    let logged = wishlist_call(
        &client,
        "log_meal",
        json!({
            "eaten_at": "2026-10-29T08:15:00+01:00",
            "portions": [{"barcode": SYNTHETIC_BARCODE, "grams": 200, "estimates": {"dietary_fibre": 1}}],
            "request_id": "mcp-product-meal-1",
        }),
    )
    .await;
    assert_eq!(logged.is_error, Some(false), "{logged:?}");
    let portion = logged.structured_content.unwrap()["meal_log"]["portions"][0].clone();
    assert_eq!(
        (
            &portion["product"]["name"],
            &portion["nutrients"]["energy_kcal"],
            &portion["nutrients"]["dietary_fibre"]
        ),
        (
            &json!("Vanilla Skyr"),
            &json!({"value": 152.0, "unit": "kcal", "source": "label"}),
            &json!({"value": 2.0, "unit": "g", "source": "estimate"})
        )
    );
    assert_eq!(
        direct("/api/nutrition/meal-logs?day=2026-10-29".into()).await["meal_logs"][0]["portions"]
            [0],
        portion
    );

    for (tool, arguments, path) in [
        (
            "lookup_product",
            json!({"barcode": "2005550001116"}),
            "/api/nutrition/products/2005550001116",
        ),
        (
            "lookup_product",
            json!({"barcode": "123"}),
            "/api/nutrition/products/123",
        ),
    ] {
        let result = wishlist_call(&client, tool, arguments).await;
        assert_eq!(result.is_error, Some(true), "{tool}");
        assert_eq!(
            result.structured_content,
            Some(direct(path.into()).await),
            "{path}"
        );
    }
    let unknown_food = wishlist_call(
        &client,
        "link_product",
        json!({"barcode": SYNTHETIC_BARCODE, "food_id": 424_242, "request_id": "mcp-link-2"}),
    )
    .await;
    assert_eq!(unknown_food.is_error, Some(true));
    assert_eq!(unknown_food.structured_content.unwrap()["status"], 404);
    client.cancel().await.unwrap();
}
