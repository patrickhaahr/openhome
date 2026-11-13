mod common;

use common::{
    API_KEY, connect, food_database, start_api, start_api_with_foods, start_mcp, wishlist_call,
};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

#[tokio::test]
#[expect(
    clippy::too_many_lines,
    reason = "one real MCP workflow owns logging, receipt replay, day reads, correction and deletion"
)]
async fn meal_log_tools_forward_logs_receipts_days_and_errors_unchanged() {
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
        instructions.contains("explicit eaten_at")
            && instructions.contains("Split a mixed dish")
            && instructions.contains("reuse the identical arguments and request_id"),
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

    let payload = json!({
        "eaten_at": "2026-10-29T08:15:00+01:00",
        "note": "after the run",
        "portions": [{"food_id": 1693, "grams": 150}],
        "request_id": "mcp-meal-1",
    });
    let logged = wishlist_call(&client, "log_meal", payload.clone()).await;
    assert_eq!(logged.is_error, Some(false), "{logged:?}");
    let logged = logged.structured_content.unwrap();
    assert_eq!(logged["replayed"], false);
    let meal_log = logged["meal_log"].clone();
    assert_eq!(meal_log["day"], "2026-10-29");
    assert_eq!(
        meal_log["portions"][0]["nutrients"]["energy_kcal"],
        json!({"value": 93.0, "unit": "kcal", "source": "food"})
    );
    assert_eq!(
        wishlist_call(&client, "log_meal", payload.clone())
            .await
            .structured_content,
        Some(json!({"meal_log": meal_log, "replayed": true}))
    );
    let mut changed = payload.clone();
    changed["portions"][0]["grams"] = json!(200);
    let conflict = wishlist_call(&client, "log_meal", changed).await;
    assert_eq!(conflict.is_error, Some(true));
    assert_eq!(conflict.structured_content.unwrap()["status"], 409);

    let day = wishlist_call(&client, "meal_logs", json!({"day": "2026-10-29"})).await;
    let expected = direct("/api/nutrition/meal-logs?day=2026-10-29".into()).await;
    assert_eq!(expected["meal_logs"], json!([meal_log]));
    assert_eq!(day.structured_content, Some(expected));
    let range = wishlist_call(
        &client,
        "meal_logs",
        json!({"from": "2026-10-28", "to": "2026-10-29"}),
    )
    .await;
    assert_eq!(
        range.structured_content,
        Some(direct("/api/nutrition/meal-logs?from=2026-10-28&to=2026-10-29".into()).await)
    );
    let nutrition_day = wishlist_call(&client, "nutrition_day", json!({"day": "2026-10-29"})).await;
    let expected = direct("/api/nutrition/days/2026-10-29".into()).await;
    assert_eq!(
        expected["totals"]["protein"],
        json!({
            "value": 16.5, "unit": "g", "unknown_portions": 0,
            "food": 16.5, "supplements": null,
        })
    );
    assert_eq!(nutrition_day.structured_content, Some(expected));

    let id = meal_log["id"].as_str().unwrap();
    let updated = wishlist_call(
        &client,
        "update_meal_log",
        json!({"meal_log_id": id, "eaten_at": "2026-10-28T21:00:00+01:00", "note": null}),
    )
    .await
    .structured_content
    .unwrap();
    assert_eq!(
        (&updated["day"], &updated["note"], &updated["portions"]),
        (&json!("2026-10-28"), &Value::Null, &meal_log["portions"])
    );
    let null_portions = wishlist_call(
        &client,
        "update_meal_log",
        json!({"meal_log_id": id, "portions": null}),
    )
    .await;
    assert_eq!(null_portions.is_error, Some(true));

    let target = json!({"meal_log_id": id});
    assert_eq!(
        wishlist_call(&client, "delete_meal_log", target.clone())
            .await
            .structured_content,
        Some(json!({"deleted": true}))
    );
    for (tool, arguments) in [
        ("delete_meal_log", target.clone()),
        (
            "update_meal_log",
            json!({"meal_log_id": id, "note": "late"}),
        ),
        ("log_meal", payload),
    ] {
        assert_eq!(
            wishlist_call(&client, tool, arguments).await.is_error,
            Some(true),
            "{tool}"
        );
    }
    for (tool, arguments, path) in [
        (
            "meal_logs",
            json!({"day": "2026-02-30"}),
            "/api/nutrition/meal-logs?day=2026-02-30",
        ),
        (
            "nutrition_day",
            json!({"day": "yesterday"}),
            "/api/nutrition/days/yesterday",
        ),
    ] {
        let result = wishlist_call(&client, tool, arguments).await;
        assert_eq!(result.is_error, Some(true), "{tool}");
        assert_eq!(
            result.structured_content,
            Some(direct(path.into()).await),
            "{tool}"
        );
    }

    let importing = start_api_with_foods(food_database(false).await).await;
    let client = connect(&start_mcp(&importing, API_KEY).await, API_KEY)
        .await
        .unwrap();
    for (tool, arguments) in [
        (
            "log_meal",
            json!({
                "eaten_at": "2026-10-29T08:15:00+01:00",
                "portions": [{"food_id": 1693, "grams": 150}],
                "request_id": "mcp-meal-2",
            }),
        ),
        ("nutrition_day", json!({"day": "2026-10-29"})),
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

/// A synthetic 1x1 PNG, base64-encoded.
const PIXEL_PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";

#[tokio::test]
async fn meal_log_photos_forward_unchanged_including_a_full_size_payload() {
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
        instructions.contains("pass the image file as base64 in photo"),
        "{instructions}"
    );
    let http = reqwest::Client::new();
    let day = || async {
        http.get(format!("{api}/api/nutrition/meal-logs?day=2026-10-29"))
            .bearer_auth(API_KEY)
            .send()
            .await
            .unwrap()
            .json::<Value>()
            .await
            .unwrap()["meal_logs"]
            .clone()
    };
    let meal = |request_id: &str, photo: &str| {
        json!({
            "eaten_at": "2026-10-29T08:15:00+01:00",
            "portions": [{"food_id": 1693, "grams": 150}],
            "request_id": request_id,
            "photo": photo,
        })
    };

    let logged = wishlist_call(&client, "log_meal", meal("mcp-photo-1", PIXEL_PNG)).await;
    assert_eq!(logged.is_error, Some(false), "{logged:?}");
    let meal_log = logged.structured_content.unwrap()["meal_log"].clone();
    let photo = meal_log["photo"].clone();
    assert_eq!(
        (
            &photo["width"],
            &photo["height"],
            &photo["thumbnail"]["width"]
        ),
        (&json!(1), &json!(1), &json!(1))
    );
    assert_eq!(day().await, json!([meal_log]));

    let id = meal_log["id"].as_str().unwrap();
    let replaced = wishlist_call(
        &client,
        "update_meal_log",
        json!({"meal_log_id": id, "photo": PIXEL_PNG}),
    )
    .await
    .structured_content
    .unwrap();
    assert_ne!(replaced["photo"]["id"], photo["id"]);
    let removed = wishlist_call(
        &client,
        "update_meal_log",
        json!({"meal_log_id": id, "photo": null}),
    )
    .await
    .structured_content
    .unwrap();
    assert_eq!(removed["photo"], Value::Null);
    assert_eq!(day().await, json!([removed]));

    // A full 10 MB photo is about 14 MB of base64, above rmcp's 4 MiB default: it passes the
    // adapter and reaches the API, which rejects these bytes as no image. One byte more is the
    // API's 413. "/9j/" encodes a JPEG signature and "QUFB" three more bytes.
    for (repeats, status) in [(3_495_252, 400), (3_495_253, 413)] {
        let photo = format!("/9j/{}", "QUFB".repeat(repeats));
        let rejected = wishlist_call(&client, "log_meal", meal("mcp-photo-2", &photo)).await;
        assert_eq!(rejected.is_error, Some(true));
        let error = rejected.structured_content.unwrap();
        assert_eq!(error["status"], status, "{error}");
    }
    assert_eq!(day().await, json!([removed]));
    client.cancel().await.unwrap();
}
