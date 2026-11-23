use crate::support::{Api, JUICE, SKYR, UNKNOWN, found, label, skyr_label};
use axum::http::StatusCode;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use wiremock::ResponseTemplate;

#[tokio::test]
async fn a_lookup_previews_normalised_label_values_with_suggested_foods_and_stores_nothing() {
    let api = Api::start().await;
    api.provider(SKYR, found(&skyr_label()), 2).await;

    let (status, preview) = api.lookup(SKYR).await;

    assert_eq!(status, StatusCode::OK, "{preview}");
    assert_eq!(preview["linked"], false);
    assert_eq!(
        preview["product"],
        json!({
            "barcode": SKYR,
            "name": "Vanilla Skyr",
            "brand": "Synthetic Dairy",
            "nova_class": 3,
            "label_per_100g": [
                label("energy_kj", 334.72, "kJ"),
                label("energy_kcal", 80.0, "kcal"),
                label("protein", 10.0, "g"),
                label("available_carbohydrate", 8.0, "g"),
                label("fat", 0.5, "g"),
                label("salt_labelling", 0.1, "g"),
                label("vitamin_d", 1.5, "µg"),
                label("sodium", 40.0, "mg"),
                label("calcium", 120.0, "mg"),
                label("sugars", 7.5, "g"),
                label("saturated_fat", 0.3, "g"),
            ],
            "food": null,
        })
    );
    assert_eq!(preview["suggested_foods"][0]["food_id"], 1693, "{preview}");
    assert_eq!(
        preview["source"],
        json!({
            "name": "Open Food Facts",
            "url": "https://world.openfoodfacts.org",
            "credit": "Product data © Open Food Facts contributors",
            "licence": "ODbL",
        })
    );
    let (_, again) = api.lookup(SKYR).await;
    assert_eq!(again["linked"], false);
}

#[tokio::test]
async fn linking_stores_the_product_once_and_a_linked_barcode_skips_open_food_facts() {
    let api = Api::start().await;
    api.provider(SKYR, found(&skyr_label()), 2).await;
    let (_, preview) = api.lookup(SKYR).await;

    let (status, linked) = api.link(SKYR, 1693, "link-skyr").await;

    assert_eq!(status, StatusCode::CREATED, "{linked}");
    assert_eq!(linked["replayed"], false);
    let mut product = preview["product"].clone();
    product["food"] =
        json!({"food_id": 1693, "name_da": "Skyr, 0.2 % fedt", "name_en": "Skyr, 0.2 % fat"});
    assert_eq!(linked["product"], product);
    let (status, lookup) = api.lookup(SKYR).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        (
            &lookup["linked"],
            &lookup["product"],
            &lookup["suggested_foods"]
        ),
        (&json!(true), &product, &json!([]))
    );

    let (status, replay) = api.link(SKYR, 1693, "link-skyr").await;
    assert_eq!(
        (status, &replay["replayed"]),
        (StatusCode::OK, &json!(true))
    );
    assert_eq!(replay["product"], product);
    let (status, _) = api.link(SKYR, 907, "link-skyr").await;
    assert_eq!(status, StatusCode::CONFLICT);

    let (status, relinked) = api.link(SKYR, 907, "relink-skyr").await;
    assert_eq!(status, StatusCode::OK, "{relinked}");
    assert_eq!(relinked["replayed"], false);
    assert_eq!(relinked["product"]["food"]["food_id"], 907);
    assert_eq!(
        relinked["product"]["label_per_100g"],
        product["label_per_100g"]
    );
    assert_eq!(api.lookup(SKYR).await.1["product"], relinked["product"]);
    let (status, replayed) = api.link(SKYR, 1693, "link-skyr").await;
    assert_eq!(status, StatusCode::OK, "{replayed}");
    assert_eq!(replayed["product"], product);
    assert_eq!(api.lookup(SKYR).await.1["product"], relinked["product"]);
    assert_eq!(api.provider_reads().await, 2);
}

#[tokio::test]
async fn a_legacy_receipt_directs_the_client_to_look_the_product_up_first() {
    let api = Api::start().await;
    api.provider(SKYR, found(&skyr_label()), 1).await;
    let (status, linked) = api.link(SKYR, 1693, "link-skyr").await;
    assert_eq!(status, StatusCode::CREATED, "{linked}");
    sqlx::query("UPDATE product_link_receipts SET result = NULL WHERE request_id = 'link-skyr'")
        .execute(&api.state.db)
        .await
        .unwrap();

    let (status, body) = api.link(SKYR, 1693, "link-skyr").await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert!(
        body["error"]
            .as_str()
            .unwrap()
            .contains("no original result"),
        "{body}"
    );
    let (status, lookup) = api.lookup(SKYR).await;
    assert_eq!((status, &lookup["linked"]), (StatusCode::OK, &json!(true)));
}

#[tokio::test]
async fn energy_salt_and_alcohol_are_normalised_from_whatever_the_label_gives() {
    let api = Api::start().await;
    let juice = json!({
        "code": JUICE,
        "product_name": "Sparkling Apple",
        "nova_group": "4",
        "nutriments": {
            "energy_100g": 1000, "sodium_100g": 0.2, "alcohol_100g": 5,
            "fiber_100g": "1.2", "iron_100g": 0.0021, "vitamin-a_100g": 0.00006,
            "sugars_100g": 140, "fat_100g": -1, "proteins_100g": "n/a",
        },
    });
    api.provider(JUICE, found(&juice), 1).await;

    let (status, preview) = api.lookup(JUICE).await;

    assert_eq!(status, StatusCode::OK, "{preview}");
    assert_eq!(
        (
            &preview["product"]["brand"],
            &preview["product"]["nova_class"]
        ),
        (&Value::Null, &json!(4))
    );
    assert_eq!(
        preview["product"]["label_per_100g"],
        json!([
            label("energy_kj", 1000.0, "kJ"),
            label("energy_kcal", 239.006, "kcal"),
            label("dietary_fibre", 1.2, "g"),
            label("alcohol", 3.945, "g"),
            label("salt_labelling", 0.5, "g"),
            label("vitamin_a", 60.0, "µg RE"),
            label("sodium", 200.0, "mg"),
            label("iron", 2.1, "mg"),
        ])
    );
}

#[tokio::test]
async fn unknown_barcodes_are_not_found_and_malformed_barcodes_never_reach_open_food_facts() {
    let api = Api::start().await;
    api.provider(
        UNKNOWN,
        ResponseTemplate::new(404).set_body_json(json!({
            "code": UNKNOWN, "status": 0, "status_verbose": "product not found",
        })),
        2,
    )
    .await;
    api.provider(
        "0012345678905",
        ResponseTemplate::new(200).set_body_json(json!({"code": "0012345678905", "status": 0})),
        1,
    )
    .await;

    for barcode in [UNKNOWN, "012345678905"] {
        let (status, body) = api.lookup(barcode).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{barcode}: {body}");
        assert_eq!(body["code"], "product_not_found", "{barcode}");
        assert!(
            body["error"].as_str().unwrap().contains("closest Food"),
            "{body}"
        );
    }
    let (status, _) = api.link(UNKNOWN, 1693, "link-unknown").await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    for barcode in [
        "12345",
        "2001234567890",
        "200123456789a",
        "20012345678931",
        "%20",
    ] {
        let (status, body) = api.lookup(barcode).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{barcode}: {body}");
    }
    let (status, _) = api.link("2001234567890", 1693, "link-bad-digit").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(api.provider_reads().await, 3);
}

#[tokio::test]
async fn rate_limits_and_failures_are_unavailable_once_without_upstream_text() {
    let api = Api::start().await;
    let barcodes = [
        "2000000000015",
        "2001111111119",
        "2002222222220",
        "2003333333331",
    ];
    let templates = [
        ResponseTemplate::new(429).set_body_string("upstream detail: slow down"),
        ResponseTemplate::new(500).set_body_string("upstream detail: stack trace"),
        ResponseTemplate::new(200).set_body_string("upstream detail: <html>"),
        found(&json!({"code": barcodes[3], "product_name": "x".repeat(300 * 1024)})),
    ];
    for (index, (barcode, template)) in barcodes.into_iter().zip(templates).enumerate() {
        api.provider(barcode, template, if index == 0 { 2 } else { 1 })
            .await;
    }

    for barcode in barcodes {
        let (status, body) = api.lookup(barcode).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{barcode}: {body}");
        assert_eq!(body["code"], "open_food_facts_unavailable");
        assert!(!body.to_string().contains("upstream"), "{body}");
    }
    let (status, _) = api.link(barcodes[0], 1693, "link-limited").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
}
