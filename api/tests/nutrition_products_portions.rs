//! Product portions through the nutrition routes: label values, linked Foods, relinking and NOVA
//! classes, with Open Food Facts mocked by `wiremock`, over a real `app.db` and a real `food.db`.

mod common;
#[path = "nutrition_support/products.rs"]
mod support;

use axum::{http::Method, http::StatusCode};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use support::{Api, SKYR, UNKNOWN, amount, found, meal, nutrient, skyr_label};

#[tokio::test]
async fn a_product_portion_takes_label_values_first_and_its_linked_food_fills_the_gaps() {
    let api = Api::start().await;
    api.linked_skyr(1693).await;

    let meal_log = api
        .logged(json!([{"barcode": SKYR, "grams": 200}]), "skyr-1")
        .await;

    let portion = &meal_log["portions"][0];
    assert_eq!(
        (
            &portion["food_id"],
            &portion["product"],
            &portion["name_en"],
            &portion["grams"]
        ),
        (
            &json!(1693),
            &json!({"barcode": SKYR, "name": "Vanilla Skyr", "brand": "Synthetic Dairy"}),
            &json!("Skyr, 0.2 % fat"),
            &json!(200.0)
        )
    );
    // The label's energy and macronutrients win; the Food has the fibre and alcohol it lacks.
    assert_eq!(
        portion["nutrients"],
        json!({
            "energy_kcal": amount(Some(160.0), "kcal", Some("label")),
            "protein": amount(Some(20.0), "g", Some("label")),
            "available_carbohydrate": amount(Some(16.0), "g", Some("label")),
            "fat": amount(Some(1.0), "g", Some("label")),
            "dietary_fibre": amount(Some(0.0), "g", Some("food")),
            "alcohol": amount(Some(0.0), "g", Some("food")),
        })
    );
    let day = api.day().await;
    // The label's vitamin D (1.5 µg per 100 g, from grams) wins over the Food's 0 µg, and its
    // kJ comes from the label's kcal rather than the Food's 260 kJ.
    assert_eq!(
        (
            &nutrient(&day, "vitamin_d")["value"],
            &nutrient(&day, "energy_kj")["value"],
            &nutrient(&day, "sodium")["value"],
            &nutrient(&day, "sodium")["unit"],
        ),
        (&json!(3.0), &json!(669.44), &json!(80.0), &json!("mg"))
    );
    assert_eq!(day["label_source"]["licence"], "ODbL");
    assert_eq!(day["meal_logs"], json!([meal_log]));
    // A summary over the day credits both sources too.
    let (status, summary) = api
        .send(
            Method::GET,
            "/api/nutrition/summary?from=2026-10-29&to=2026-10-29&aggregation=day",
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{summary}");
    assert_eq!(
        (
            &summary["sources"][0]["credit"],
            &summary["label_source"]["credit"]
        ),
        (
            &json!("© The Danish Food Composition Database, version 1"),
            &json!("Product data © Open Food Facts contributors")
        )
    );
    assert_eq!(api.provider_reads().await, 1);
}

#[tokio::test]
async fn estimates_fill_only_the_values_no_source_has() {
    let api = Api::start().await;
    api.linked_skyr(1693).await;
    // The chicken has no carbohydrate, fibre or alcohol value; nothing has EPA for the skyr.
    let portions = json!([
        {"food_id": 907, "grams": 200, "estimates": {"dietary_fibre": 0.5, "available_carbohydrate": 1}},
        {"barcode": SKYR, "grams": 100, "estimates": {"epa": 0.002}},
    ]);

    let meal_log = api.logged(portions.clone(), "estimated-1").await;

    let chicken = &meal_log["portions"][0]["nutrients"];
    assert_eq!(
        (
            &chicken["dietary_fibre"],
            &chicken["available_carbohydrate"],
            &chicken["protein"],
            &chicken["alcohol"],
        ),
        (
            &amount(Some(1.0), "g", Some("estimate")),
            &amount(Some(2.0), "g", Some("estimate")),
            &amount(Some(42.0), "g", Some("food")),
            &amount(None, "g", None),
        )
    );
    // The chicken's own EPA (0.01 g per 100 g) and the skyr's estimate add up.
    assert_eq!(nutrient(&api.day().await, "epa")["value"], json!(0.022));
    let (status, replay) = api.log(meal(&portions, "estimated-1")).await;
    assert_eq!((status, &replay["meal_log"]), (StatusCode::OK, &meal_log));
    let mut changed = portions;
    changed[1]["estimates"]["epa"] = json!(0.003);
    assert_eq!(
        api.log(meal(&changed, "estimated-1")).await.0,
        StatusCode::CONFLICT
    );

    for (estimates, refused) in [
        (json!({"protein": 20}), "the Food has protein"),
        (json!({"fat": 1}), "the Food has fat"),
        (json!({"unobtainium": 1}), "unknown key"),
        (json!({"waste": 10}), "not a per-100 g value"),
        (json!({"dietary_fibre": -1}), "negative"),
        (json!({"dietary_fibre": 101}), "more than 100 g in 100 g"),
    ] {
        let (status, body) = api
            .log(meal(
                &json!([{"food_id": 907, "grams": 100, "estimates": estimates}]),
                "refused-estimate",
            ))
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{refused}: {body}");
    }
    let (status, body) = api
        .log(meal(
            &json!([{"barcode": SKYR, "grams": 100, "estimates": {"energy_kcal": 70}}]),
            "refused-label-estimate",
        ))
        .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "the label has energy: {body}"
    );
    assert_eq!(api.day().await["meal_logs"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn relinking_a_product_changes_only_future_portions() {
    let api = Api::start().await;
    api.linked_skyr(1693).await;
    let before = api
        .logged(json!([{"barcode": SKYR, "grams": 100}]), "before")
        .await;

    let (status, _) = api.link(SKYR, 907, "relink-skyr").await;
    assert_eq!(status, StatusCode::OK);
    let after = api
        .logged(json!([{"barcode": SKYR, "grams": 100}]), "after")
        .await;

    // The past Portion keeps the skyr Food's fibre; the new one takes the chicken's gaps.
    let day = api.day().await;
    assert_eq!(day["meal_logs"], json!([before, after]));
    assert_eq!(
        (
            &before["portions"][0]["food_id"],
            &before["portions"][0]["nutrients"]["dietary_fibre"],
        ),
        (&json!(1693), &amount(Some(0.0), "g", Some("food")))
    );
    assert_eq!(
        (
            &after["portions"][0]["food_id"],
            &after["portions"][0]["nutrients"]["dietary_fibre"],
            &after["portions"][0]["nutrients"]["energy_kcal"],
        ),
        (
            &json!(907),
            &amount(None, "g", None),
            &amount(Some(80.0), "kcal", Some("label")),
        )
    );
    // Replacing a past Portion snapshots the Product as it is linked now.
    let id = before["id"].as_str().unwrap();
    let (status, replaced) = api
        .send(
            Method::PATCH,
            &format!("/api/nutrition/meal-logs/{id}"),
            Some(json!({"portions": [{"barcode": SKYR, "grams": 150}]})),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{replaced}");
    assert_eq!(replaced["portions"][0]["food_id"], 907);
    assert_eq!(
        replaced["portions"][0]["nutrients"]["protein"],
        amount(Some(15.0), "g", Some("label"))
    );
    assert_eq!(api.provider_reads().await, 1);
}

#[tokio::test]
async fn unlinked_or_ambiguous_product_portions_are_refused_without_writing() {
    let api = Api::start().await;

    for (portion, expected) in [
        (
            json!({"barcode": UNKNOWN, "grams": 100}),
            StatusCode::NOT_FOUND,
        ),
        (
            json!({"barcode": "2001234567890", "grams": 100}),
            StatusCode::BAD_REQUEST,
        ),
        (
            json!({"barcode": SKYR, "food_id": 1693, "grams": 100}),
            StatusCode::BAD_REQUEST,
        ),
        (json!({"grams": 100}), StatusCode::BAD_REQUEST),
    ] {
        let (status, body) = api.log(meal(&json!([portion]), "refused")).await;
        assert_eq!(status, expected, "{portion}: {body}");
        if status == StatusCode::NOT_FOUND {
            assert!(
                body["error"].as_str().unwrap().contains("link_product"),
                "{body}"
            );
        }
    }
    assert_eq!(api.day().await["meal_logs"], json!([]));
    assert_eq!(api.provider_reads().await, 0);
}

#[tokio::test]
async fn the_nova_1_share_counts_classified_kcal_with_products_taking_the_open_food_facts_class() {
    let api = Api::start().await;
    api.linked_skyr(1693).await;
    let (_, preview) = api.lookup(SKYR).await;
    assert_eq!(preview["product"]["nova_class"], 3);

    // Nothing classified yet: no share rather than zero.
    api.logged(json!([{"food_id": 2, "grams": 100}]), "apple")
        .await;
    assert_eq!(
        api.day().await["nova"],
        json!({
            "nova_1_share": null, "nova_1_kcal": 0.0, "classified_kcal": 0.0,
            "unclassified_portions": 1,
        })
    );

    // Classifying the skyr Food leaves the Product on Open Food Facts' class.
    for (food_id, nova_class) in [(1693, 1), (3, 1), (907, 1)] {
        let (status, body) = api
            .send(
                Method::PUT,
                &format!("/api/nutrition/foods/{food_id}/nova-class"),
                Some(json!({"nova_class": nova_class})),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
    }
    let meal_log = api
        .logged(
            json!([
                {"barcode": SKYR, "grams": 200},
                {"food_id": 3, "grams": 100},
                {"food_id": 907, "grams": 100},
            ]),
            "breakfast",
        )
        .await;
    let classes: Vec<&Value> = meal_log["portions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|portion| &portion["nova_class"])
        .collect();
    assert_eq!(classes, [&json!(3), &json!(1), &json!(1)]);

    // Banana 90 kcal and chicken 150 kcal are Class 1 of 400 classified kcal, beside the
    // 160 kcal Class 3 skyr; the unclassified 50 kcal apple is left out.
    assert_eq!(
        api.day().await["nova"],
        json!({
            "nova_1_share": 0.6, "nova_1_kcal": 240.0, "classified_kcal": 400.0,
            "unclassified_portions": 1,
        })
    );
}

#[tokio::test]
async fn a_product_without_an_open_food_facts_class_stays_unclassified_when_its_food_is_classified()
{
    let api = Api::start().await;
    let mut unclassified = skyr_label();
    unclassified["nova_group"] = Value::Null;
    api.provider(SKYR, found(&unclassified), 1).await;
    let (status, linked) = api.link(SKYR, 1693, "link-skyr").await;
    assert_eq!(status, StatusCode::CREATED, "{linked}");
    api.logged(json!([{"barcode": SKYR, "grams": 100}]), "skyr")
        .await;

    let (status, classified) = api
        .send(
            Method::PUT,
            "/api/nutrition/foods/1693/nova-class",
            Some(json!({"nova_class": 1})),
        )
        .await;

    assert_eq!(status, StatusCode::OK, "{classified}");
    assert_eq!(classified["classified_portions"], 0);
    let later = api
        .logged(json!([{"barcode": SKYR, "grams": 100}]), "skyr-2")
        .await;
    assert_eq!(later["portions"][0]["nova_class"], Value::Null);
    let day = api.day().await;
    assert_eq!(
        day["meal_logs"][0]["portions"][0]["nova_class"],
        Value::Null
    );
    assert_eq!(day["nova"]["nova_1_share"], Value::Null);
}
