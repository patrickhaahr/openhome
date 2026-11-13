//! MCP-to-API contract: the adapter in front of the real OpenHome API training routes, driven by
//! an MCP Streamable HTTP client.

mod common;

use axum::Router;
use common::{
    API_KEY, api_get, connect, recovery_on_day, serve, start_api, start_mcp, wishlist_call,
};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::sync::Arc;

const INITIALIZE: &str = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"contract-test","version":"1.0"}}}"#;

#[tokio::test]
#[expect(
    clippy::too_many_lines,
    reason = "one discovery contract verifies training and Wishlist tool schemas and annotations"
)]
async fn discovers_read_only_training_and_additive_wishlist_tools() {
    let mcp = start_mcp(&start_api().await, API_KEY).await;
    let client = connect(&mcp, API_KEY).await.unwrap();

    let info = client.peer_info().unwrap();
    let server = info.server_info.as_ref().unwrap();
    assert_eq!(server.name, "openhome-mcp");
    assert_eq!(server.version, env!("CARGO_PKG_VERSION"));

    let tools = client.list_all_tools().await.unwrap();
    let names: Vec<&str> = tools.iter().map(|tool| tool.name.as_ref()).collect();
    assert_eq!(
        names,
        [
            "add_important_date",
            "add_supplement",
            "add_watchlist_entry",
            "add_wishlist_item",
            "classify_food",
            "create_wishlist",
            "delete_important_date",
            "delete_meal",
            "delete_meal_log",
            "delete_supplement",
            "delete_watchlist_entry",
            "delete_wishlist_item",
            "exercise_history",
            "food",
            "household_users",
            "important_date",
            "important_dates",
            "link_product",
            "log_meal",
            "lookup_product",
            "meal_logs",
            "meals",
            "metric_trend",
            "noop_workout_detail",
            "nutrition_day",
            "nutrition_goal",
            "nutrition_summary",
            "recovery_on_day",
            "save_meal",
            "search_cards",
            "search_foods",
            "set_nutrition_goal",
            "sleep_recent",
            "supplements",
            "training_context",
            "update_important_date",
            "update_meal",
            "update_meal_log",
            "update_supplement",
            "update_watchlist_entry",
            "update_wishlist_item",
            "watchlist_entries",
            "watchlist_entry",
            "wishlist_item",
            "wishlist_items",
            "wishlist_url_metadata",
            "wishlists",
            "workouts_on_day"
        ]
    );
    for tool in &tools {
        assert_eq!(
            tool.annotations
                .as_ref()
                .and_then(|annotations| annotations.read_only_hint),
            Some(!matches!(
                tool.name.as_ref(),
                "create_wishlist"
                    | "add_wishlist_item"
                    | "update_wishlist_item"
                    | "delete_wishlist_item"
                    | "add_watchlist_entry"
                    | "delete_watchlist_entry"
                    | "update_watchlist_entry"
                    | "add_important_date"
                    | "update_important_date"
                    | "delete_important_date"
                    | "log_meal"
                    | "update_meal_log"
                    | "delete_meal_log"
                    | "save_meal"
                    | "update_meal"
                    | "delete_meal"
                    | "add_supplement"
                    | "update_supplement"
                    | "delete_supplement"
                    | "link_product"
                    | "classify_food"
                    | "set_nutrition_goal"
            )),
            "{}",
            tool.name
        );
        if matches!(
            tool.name.as_ref(),
            "create_wishlist"
                | "add_wishlist_item"
                | "add_watchlist_entry"
                | "add_important_date"
                | "log_meal"
                | "save_meal"
                | "add_supplement"
                | "link_product"
                | "classify_food"
                | "set_nutrition_goal"
        ) {
            let annotations = tool.annotations.as_ref().unwrap();
            assert_eq!(annotations.destructive_hint, Some(false));
            assert_eq!(
                annotations.idempotent_hint,
                Some(tool.name != "create_wishlist")
            );
        }
        if matches!(
            tool.name.as_ref(),
            "update_wishlist_item"
                | "delete_wishlist_item"
                | "delete_watchlist_entry"
                | "update_watchlist_entry"
                | "update_important_date"
                | "delete_important_date"
                | "update_meal_log"
                | "delete_meal_log"
                | "update_meal"
                | "delete_meal"
                | "update_supplement"
                | "delete_supplement"
        ) {
            let annotations = tool.annotations.as_ref().unwrap();
            assert_eq!(annotations.destructive_hint, Some(true));
            assert_eq!(annotations.idempotent_hint, Some(true));
        }
        if matches!(
            tool.name.as_ref(),
            "lookup_product" | "link_product" | "search_cards"
        ) {
            assert_eq!(
                tool.annotations.as_ref().unwrap().open_world_hint,
                Some(true),
                "{}",
                tool.name
            );
        }
        if tool.name == "wishlist_url_metadata" {
            let annotations = tool.annotations.as_ref().unwrap();
            assert_eq!(annotations.open_world_hint, Some(true));
            assert_eq!(annotations.destructive_hint, Some(false));
        }
        let required = match tool.name.as_ref() {
            "household_users" | "wishlists" | "watchlist_entries" | "important_dates"
            | "meal_logs" | "meals" | "supplements" | "nutrition_goal" => {
                assert!(tool.input_schema.get("required").is_none());
                continue;
            }
            "create_wishlist" => json!(["owner_id", "name"]),
            "add_wishlist_item" => json!(["title", "request_id"]),
            "add_watchlist_entry" => json!(["kind", "title", "request_id"]),
            "watchlist_entry" | "delete_watchlist_entry" | "update_watchlist_entry" => {
                json!(["owner_id", "entry_id"])
            }
            "wishlist_items" => json!(["list_id"]),
            "add_important_date" => json!(["title", "date", "kind", "person_ids", "request_id"]),
            "important_date" | "delete_important_date" | "update_important_date" => {
                json!(["date_id"])
            }
            "wishlist_url_metadata" => json!(["owner_id", "list_id", "url"]),
            "wishlist_item" | "delete_wishlist_item" | "update_wishlist_item" => {
                json!(["owner_id", "list_id", "item_id"])
            }
            "exercise_history" => json!(["exercise_name", "from_day", "to_day"]),
            "search_cards" => json!(["card_code"]),
            "search_foods" => json!(["query"]),
            "food" => json!(["food_id"]),
            "log_meal" => json!(["eaten_at", "request_id"]),
            "save_meal" => json!(["name", "request_id"]),
            "add_supplement" => json!(["name", "dose_unit", "request_id"]),
            "update_supplement" | "delete_supplement" => json!(["supplement_id"]),
            "update_meal" | "delete_meal" => json!(["meal_id"]),
            "lookup_product" => json!(["barcode"]),
            "link_product" => json!(["barcode", "food_id", "request_id"]),
            "classify_food" => json!(["food_id", "nova_class"]),
            "set_nutrition_goal" => json!(["direction"]),
            "nutrition_summary" => json!(["from", "to", "aggregation"]),
            "update_meal_log" | "delete_meal_log" => json!(["meal_log_id"]),
            "metric_trend" => json!(["metric", "from_day", "to_day"]),
            "noop_workout_detail" => json!(["source", "start"]),
            "sleep_recent" => json!(["n"]),
            "training_context" => json!(["from_day", "to_day"]),
            _ => json!(["day"]),
        };
        assert_eq!(tool.input_schema.get("required"), Some(&required));
        if tool.name == "metric_trend" {
            assert_eq!(
                tool.input_schema["properties"]["metric"]["$ref"],
                "#/$defs/TrendMetric"
            );
            assert_eq!(
                tool.input_schema["$defs"]["TrendMetric"]["enum"],
                json!(["body_weight", "sleep_duration", "resting_heart_rate", "hrv"])
            );
        }
        assert!(tool.description.as_deref().is_some_and(|description| {
            (description.contains("Europe/Copenhagen")
                || tool.name.contains("wishlist")
                || tool.name.contains("watchlist")
                || tool.name.contains("important_date")
                || tool.name.contains("food")
                || tool.name.contains("card")
                || tool.name.contains("product")
                || tool.name.contains("meal")
                || tool.name.contains("supplement")
                || tool.name.contains("nutrition_goal"))
                && !description.contains('\n')
        }));
    }

    client.cancel().await.unwrap();
}

#[tokio::test]
async fn does_not_follow_api_redirects() {
    let redirected_calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let calls = redirected_calls.clone();
    let error = json!({"error": "Recovery endpoint moved", "status": 302});
    let response_error = error.clone();
    let api = Router::new()
        .route(
            "/api/training/days/{day}/recovery",
            axum::routing::get(move || async move {
                (
                    axum::http::StatusCode::FOUND,
                    [(axum::http::header::LOCATION, "/other")],
                    axum::Json(response_error),
                )
            }),
        )
        .route(
            "/other",
            axum::routing::get(move || async move {
                calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                axum::Json(json!({"unexpected": true}))
            }),
        );
    let api_url = format!("http://{}", serve(api).await);
    let client = connect(&start_mcp(&api_url, API_KEY).await, API_KEY)
        .await
        .unwrap();

    let result = recovery_on_day(&client, "2026-09-14").await;
    assert_eq!(
        redirected_calls.load(std::sync::atomic::Ordering::SeqCst),
        0
    );
    assert_eq!(result.is_error, Some(true));
    assert_eq!(result.structured_content, Some(error));
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn rejects_dot_path_arguments() {
    let api_calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let calls = api_calls.clone();
    let api = Router::new().fallback(move || async move {
        calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        axum::http::StatusCode::NOT_FOUND
    });
    let api_url = format!("http://{}", serve(api).await);
    let client = connect(&start_mcp(&api_url, API_KEY).await, API_KEY)
        .await
        .unwrap();
    for day in [".", ".."] {
        let result = recovery_on_day(&client, day).await;
        assert_eq!(result.is_error, Some(true));
        assert_eq!(result.structured_content.unwrap()["status"], 400);
    }
    assert_eq!(api_calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn rejects_clients_without_the_api_key() {
    let mcp = start_mcp(&start_api().await, API_KEY).await;

    for authorization in [None, Some("Bearer wrong-key"), Some(API_KEY)] {
        let mut request = reqwest::Client::new()
            .post(&mcp)
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .body(INITIALIZE);
        if let Some(authorization) = authorization {
            request = request.header("authorization", authorization);
        }
        let response = request.send().await.unwrap();

        assert_eq!(response.status(), 401, "{authorization:?}");
        assert_eq!(response.headers()["www-authenticate"], "Bearer");
        assert_eq!(
            response.json::<Value>().await.unwrap(),
            json!({"error": "Missing or invalid API key", "status": 401})
        );
    }
    assert!(connect(&mcp, "wrong-key").await.is_err());
}

#[tokio::test]
async fn returns_api_errors_as_tool_errors() {
    let api = start_api().await;
    let client = connect(&start_mcp(&api, API_KEY).await, API_KEY)
        .await
        .unwrap();

    let invalid = recovery_on_day(&client, "2026-02-30").await;
    assert_eq!(invalid.is_error, Some(true));
    assert_eq!(
        invalid.structured_content,
        Some(json!({
            "error": "Invalid day '2026-02-30': must be a Europe/Copenhagen calendar date written \
                      YYYY-MM-DD",
            "status": 400,
        }))
    );
    assert_eq!(
        invalid.structured_content,
        Some(api_get(&api, "2026-02-30").await)
    );

    // The argument stays one path segment, so it cannot reach another API route.
    let escape = recovery_on_day(&client, "../../health").await;
    assert_eq!(escape.is_error, Some(true));
    assert_eq!(escape.structured_content.unwrap()["status"], 400);
    client.cancel().await.unwrap();

    // The API rejecting the adapter's key reaches the caller as the API's 401.
    let misconfigured = connect(&start_mcp(&api, "wrong-key").await, API_KEY)
        .await
        .unwrap();
    let rejected = recovery_on_day(&misconfigured, "2026-09-14").await;
    assert_eq!(rejected.is_error, Some(true));
    assert_eq!(
        rejected.structured_content,
        Some(json!({"error": "Missing or invalid API key", "status": 401}))
    );
    misconfigured.cancel().await.unwrap();

    let unreachable = connect(&start_mcp("http://127.0.0.1:1", API_KEY).await, API_KEY)
        .await
        .unwrap();
    let down = recovery_on_day(&unreachable, "2026-09-14").await;
    assert_eq!(down.is_error, Some(true));
    assert_eq!(
        down.structured_content,
        Some(json!({"error": "The OpenHome API could not be reached", "status": null}))
    );
    unreachable.cancel().await.unwrap();
}

#[tokio::test]
async fn unexpected_no_content_never_confirms_a_read_or_item_write() {
    let upstream = Router::new().route(
        "/api/{*path}",
        axum::routing::any(|| async { axum::http::StatusCode::NO_CONTENT }),
    );
    let api = format!("http://{}", serve(upstream).await);
    let client = connect(&start_mcp(&api, API_KEY).await, API_KEY)
        .await
        .unwrap();
    for (tool, args) in [
        (
            "training_context",
            json!({"from_day":"2026-09-14","to_day":"2026-09-14"}),
        ),
        (
            "add_wishlist_item",
            json!({"title":"A jacket","request_id":"missing-receipt"}),
        ),
        (
            "update_watchlist_entry",
            json!({"owner_id":"owner","entry_id":"entry","status":"watching"}),
        ),
        (
            "update_wishlist_item",
            json!({"owner_id":"owner","list_id":"list","item_id":"item","quantity":2}),
        ),
    ] {
        let result = wishlist_call(&client, tool, args).await;
        assert_eq!(
            result.is_error,
            Some(true),
            "{tool} must not confirm an empty result"
        );
        assert_eq!(result.structured_content.unwrap()["status"], 204);
    }
    client.cancel().await.unwrap();
}
