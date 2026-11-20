mod common;
#[path = "nutrition_support/meal_logs.rs"]
mod support;

use axum::http::StatusCode;
use common::food_workbook::second_dataset;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use support::{Nutrition, at, breakfast};

#[tokio::test]
async fn a_food_nova_class_is_set_changed_read_back_and_kept_only_for_its_dataset_version() {
    let mut api = Nutrition::start().await;
    assert_eq!(
        api.read_class("skyr", 1693).await,
        (Value::Null, Value::Null)
    );

    let (status, classified) = api.classify(1693, json!(3)).await;
    assert_eq!(status, StatusCode::OK, "{classified}");
    assert_eq!(
        classified,
        json!({
            "food_id": 1693, "name_da": "Skyr, 0.2 % fedt", "name_en": "Skyr, 0.2 % fat",
            "nova_class": 3, "classified_portions": 0,
        })
    );
    assert_eq!(api.read_class("skyr", 1693).await, (json!(3), json!(3)));
    let (status, changed) = api.classify(1693, json!(1)).await;
    assert_eq!(status, StatusCode::OK, "{changed}");
    assert_eq!(api.read_class("skyr", 1693).await, (json!(1), json!(1)));
    assert_eq!(
        api.read_class("chicken", 907).await,
        (Value::Null, Value::Null)
    );

    api.rebuild().await;
    assert_eq!(api.read_class("skyr", 1693).await, (json!(1), json!(1)));

    api.reimport("2", second_dataset()).await;
    assert_eq!(
        api.read_class("skyr", 1693).await,
        (Value::Null, Value::Null)
    );
    let skyr = api.logged(at("2026-10-29T20:00:00+01:00", "skyr-v2")).await;
    assert_eq!(skyr["portions"][0]["nova_class"], Value::Null);
    let (status, _) = api.classify(1693, json!(1)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(api.read_class("skyr", 1693).await, (json!(1), json!(1)));
}

#[tokio::test]
async fn invalid_classifications_are_refused_and_change_nothing() {
    let api = Nutrition::start().await;

    for (food_id, nova_class, expected) in [
        (1693, json!(0), StatusCode::BAD_REQUEST),
        (1693, json!(5), StatusCode::BAD_REQUEST),
        (1693, json!(2.5), StatusCode::BAD_REQUEST),
        (1693, json!("1"), StatusCode::BAD_REQUEST),
        (1693, Value::Null, StatusCode::BAD_REQUEST),
        (0, json!(1), StatusCode::BAD_REQUEST),
        (4242, json!(1), StatusCode::NOT_FOUND),
    ] {
        let (status, body) = api.classify(food_id, nova_class.clone()).await;
        assert_eq!(status, expected, "{food_id} {nova_class}: {body}");
    }
    assert_eq!(
        api.read_class("skyr", 1693).await,
        (Value::Null, Value::Null)
    );
}

#[tokio::test]
async fn classifying_a_food_fills_its_unclassified_past_portions_and_later_portions_keep_their_class()
 {
    let mut api = Nutrition::start().await;
    let breakfast = api.logged(breakfast("breakfast-1")).await;
    assert_eq!(
        (
            &breakfast["portions"][0]["nova_class"],
            &breakfast["portions"][1]["nova_class"]
        ),
        (&Value::Null, &Value::Null)
    );

    let (_, classified) = api.classify(1693, json!(1)).await;
    assert_eq!(classified["classified_portions"], 1);
    let filled = &api.day("2026-10-29").await[0];
    assert_eq!(
        (
            &filled["portions"][0]["nova_class"],
            &filled["portions"][1]["nova_class"]
        ),
        (&json!(1), &Value::Null)
    );

    let (_, changed) = api.classify(1693, json!(2)).await;
    assert_eq!(changed["classified_portions"], 0);
    let later = api.logged(at("2026-10-29T20:00:00+01:00", "skyr-2")).await;
    assert_eq!(later["portions"][0]["nova_class"], 2);
    let meal_logs = api.day("2026-10-29").await;
    assert_eq!(meal_logs[0]["portions"][0]["nova_class"], 1);

    api.reimport("2", second_dataset()).await;
    let (_, chicken) = api.classify(907, json!(1)).await;
    assert_eq!(chicken["classified_portions"], 0);
    assert_eq!(
        api.day("2026-10-29").await[0]["portions"][1]["nova_class"],
        Value::Null
    );
}
