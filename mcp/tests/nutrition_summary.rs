mod common;

use common::{API_KEY, connect, start_api, start_mcp, wishlist_call};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

#[tokio::test]
async fn nutrition_goal_tools_read_and_set_the_goal_and_pass_errors_and_targets_through() {
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
        instructions.contains("set_nutrition_goal") && instructions.contains("remaining_kcal"),
        "{instructions}"
    );
    assert!(
        instructions.contains("micronutrients") && instructions.contains("cut_down"),
        "{instructions}"
    );
    let http = reqwest::Client::new();
    let direct = |method: reqwest::Method, path: &'static str, body: Option<Value>| {
        let mut request = http
            .request(method, format!("{api}{path}"))
            .bearer_auth(API_KEY);
        if let Some(body) = body {
            request = request.json(&body);
        }
        async move { request.send().await.unwrap().json::<Value>().await.unwrap() }
    };

    let goal = wishlist_call(&client, "nutrition_goal", json!({})).await;
    assert_eq!(goal.is_error, Some(false), "{goal:?}");
    assert_eq!(
        goal.structured_content.unwrap(),
        json!({
            "goal": {"direction": "lose", "rate_kg_per_week": 0.5, "effective_from": null},
            "history": [],
        })
    );

    let set = wishlist_call(
        &client,
        "set_nutrition_goal",
        json!({"direction": "lean_bulk", "rate_kg_per_week": 0.25}),
    )
    .await;
    assert_eq!(set.is_error, Some(false), "{set:?}");
    let set = set.structured_content.unwrap();
    assert_eq!(set["goal"]["direction"], "lean_bulk", "{set}");
    assert_eq!(set["goal"]["rate_kg_per_week"], 0.25, "{set}");
    assert_eq!(
        direct(reqwest::Method::GET, "/api/nutrition/goal", None).await,
        set
    );
    let maintain = wishlist_call(
        &client,
        "set_nutrition_goal",
        json!({"direction": "maintain"}),
    )
    .await;
    assert_eq!(maintain.is_error, Some(false), "{maintain:?}");
    assert_eq!(
        maintain.structured_content.unwrap()["goal"]["direction"],
        "maintain"
    );

    let invalid = json!({"direction": "lean_bulk", "rate_kg_per_week": 0.5});
    let result = wishlist_call(&client, "set_nutrition_goal", invalid.clone()).await;
    assert_eq!(result.is_error, Some(true));
    let error = result.structured_content.unwrap();
    assert_eq!(error["status"], 400, "{error}");
    assert_eq!(
        direct(reqwest::Method::PUT, "/api/nutrition/goal", Some(invalid)).await,
        error
    );

    // Without a Profile the day's targets say what they need.
    let day = wishlist_call(&client, "nutrition_day", json!({"day": "2026-10-29"}))
        .await
        .structured_content
        .unwrap();
    assert_eq!(day["targets"]["state"], "unavailable", "{day}");
    assert_eq!(
        day["targets"]["missing"],
        json!(["height", "sex", "birth_date", "body_weight"])
    );
    // So does the micronutrient panel.
    assert_eq!(day["micronutrients"]["state"], "unavailable", "{day}");
    assert_eq!(
        day["micronutrients"]["missing"],
        json!(["sex", "birth_date"])
    );
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn nutrition_summary_forwards_each_aggregation_and_range_errors_unchanged() {
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
        instructions.contains("nutrition_summary")
            && instructions.contains("missing_days")
            && instructions.contains("lasting_shortfall")
            && instructions.contains("seven_day_average_above_upper_limit"),
        "{instructions}"
    );
    let http = reqwest::Client::new();
    let direct = |path: String| {
        let request = http.get(format!("{api}{path}")).bearer_auth(API_KEY);
        async move { request.send().await.unwrap().json::<Value>().await.unwrap() }
    };
    let logged = wishlist_call(
        &client,
        "log_meal",
        json!({
            "eaten_at": "2026-10-28T08:15:00+01:00",
            "portions": [{"food_id": 1693, "grams": 150}],
            "request_id": "mcp-summary-meal",
        }),
    )
    .await;
    assert_eq!(logged.is_error, Some(false), "{logged:?}");

    for aggregation in ["day", "week", "month"] {
        let arguments =
            json!({"from": "2026-10-01", "to": "2026-10-29", "aggregation": aggregation});
        let summary = wishlist_call(&client, "nutrition_summary", arguments).await;
        assert_eq!(summary.is_error, Some(false), "{summary:?}");
        let expected = direct(format!(
            "/api/nutrition/summary?from=2026-10-01&to=2026-10-29&aggregation={aggregation}"
        ))
        .await;
        assert_eq!(expected["logged_days"], 1, "{expected}");
        // Without the Profile's sex and birth date the coverage says why.
        assert_eq!(
            expected["micronutrients"]["state"], "unavailable",
            "{expected}"
        );
        assert_eq!(summary.structured_content, Some(expected), "{aggregation}");
    }

    let too_long = wishlist_call(
        &client,
        "nutrition_summary",
        json!({"from": "2025-10-01", "to": "2026-10-29", "aggregation": "month"}),
    )
    .await;
    assert_eq!(too_long.is_error, Some(true));
    let error = too_long.structured_content.unwrap();
    assert_eq!(error["status"], 400, "{error}");
    assert_eq!(
        direct("/api/nutrition/summary?from=2025-10-01&to=2026-10-29&aggregation=month".into())
            .await,
        error
    );
    client.cancel().await.unwrap();
}
