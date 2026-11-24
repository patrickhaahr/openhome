use crate::common::food_workbook::second_dataset;
use crate::support::{Api, SKYR, UNLINKED, by_name, product_at, protein_bowl};
use axum::{http::Method, http::StatusCode};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

#[tokio::test]
async fn logging_a_meal_by_name_copies_its_items_as_portions_with_overrides_and_records_its_name() {
    let api = Api::start().await;
    let meal = api.saved(protein_bowl("bowl-1")).await;
    let (status, body) = api
        .send(
            Method::PUT,
            "/api/nutrition/foods/1693/nova-class",
            Some(json!({"nova_class": 1})),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let mut request = by_name("protein BOWL", "log-1");
    request["overrides"] = json!([{"food_id": 1693, "grams": 200}]);
    request["note"] = json!("bigger skyr today");
    let meal_log = api.logged(request).await;

    assert_eq!(meal_log["meal_name"], "Protein bowl");
    assert_eq!(meal_log["note"], "bigger skyr today");
    let portions: Vec<(&Value, &Value, &Value, &Value, &Value)> = meal_log["portions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|portion| {
            (
                &portion["food_id"],
                &portion["grams"],
                &portion["nutrients"]["energy_kcal"],
                &portion["nutrients"]["protein"]["value"],
                &portion["nova_class"],
            )
        })
        .collect();
    assert_eq!(
        portions,
        [
            (
                &json!(1693),
                &json!(200.0),
                &json!({"value": 124.0, "unit": "kcal", "source": "food"}),
                &json!(22.0),
                &json!(1)
            ),
            (
                &json!(907),
                &json!(120.0),
                &json!({"value": 180.0, "unit": "kcal", "source": "food"}),
                &json!(25.2),
                &Value::Null
            ),
        ]
    );
    assert_eq!(
        meal_log["totals"]["energy_kcal"],
        json!({
            "value": 304.0, "unit": "kcal", "unknown_portions": 0,
            "food": 304.0, "supplements": null,
        })
    );
    let day = api.day("2026-10-29").await;
    assert_eq!(day["meal_logs"], json!([meal_log]));
    assert_eq!(day["nova"]["nova_1_kcal"], 124.0);
    assert_eq!(api.meals().await, [meal]);

    let mut lunch = by_name("Protein bowl", "log-2");
    lunch["eaten_at"] = json!("2026-10-29T12:00:00+01:00");
    let lunch = api.logged(lunch).await;
    assert_eq!(lunch["portions"][0]["grams"], 150.0);
    assert_eq!(lunch["portions"][1]["grams"], 120.0);
}

#[tokio::test]
async fn editing_or_deleting_a_meal_leaves_its_meal_logs_unchanged() {
    let api = Api::start().await;
    let meal = api.saved(protein_bowl("bowl-1")).await;
    let mut request = by_name("Protein bowl", "log-1");
    request["overrides"] = json!([{"food_id": 907, "grams": 100}]);
    let breakfast = api.logged(request.clone()).await;

    let (status, edited) = api
        .update(
            &meal["id"],
            json!({"name": "Chicken plate", "portions": [{"food_id": 907, "grams": 300}]}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{edited}");

    assert_eq!(api.day("2026-10-29").await["meal_logs"], json!([breakfast]));
    let (status, replayed) = api.log(request.clone()).await;
    assert_eq!(status, StatusCode::OK, "{replayed}");
    assert_eq!(replayed["meal_log"], breakfast);
    let (status, body) = api.log(by_name("Protein bowl", "log-2")).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let mut dinner = by_name("chicken plate", "log-3");
    dinner["eaten_at"] = json!("2026-10-29T19:00:00+01:00");
    let dinner = api.logged(dinner).await;
    assert_eq!(dinner["meal_name"], "Chicken plate");
    assert_eq!(dinner["portions"].as_array().unwrap().len(), 1);
    assert_eq!(dinner["portions"][0]["grams"], 300.0);

    assert_eq!(api.delete(&meal["id"]).await, StatusCode::NO_CONTENT);

    assert_eq!(
        api.day("2026-10-29").await["meal_logs"],
        json!([breakfast, dinner])
    );
    let (status, replayed) = api.log(request).await;
    assert_eq!(status, StatusCode::OK, "{replayed}");
    assert_eq!(api.meals().await, Vec::<Value>::new());
    for status in [
        api.update(&meal["id"], json!({"name": "Gone"})).await.0,
        api.delete(&meal["id"]).await,
    ] {
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
}

#[tokio::test]
async fn a_meal_references_a_linked_product_by_barcode_so_a_relink_reaches_its_next_log() {
    let api = Api::start().await;
    let unlinked = json!({
        "name": "Skyr snack",
        "portions": [{"barcode": UNLINKED, "grams": 100}],
        "request_id": "snack-0",
    });
    let (status, body) = api.save(unlinked).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert!(
        body["error"].as_str().unwrap().contains("link_product"),
        "{body}"
    );
    assert_eq!(api.meals().await, Vec::<Value>::new());
    api.link(SKYR, 1693, "link-1").await;

    let meal = api
        .saved(json!({
            "name": "Skyr snack",
            "portions": [{"barcode": SKYR, "grams": 100}, {"food_id": 3, "grams": 120}],
            "request_id": "snack-1",
        }))
        .await;

    assert_eq!(
        meal["portions"][0],
        json!({
            "food_id": null, "barcode": SKYR, "grams": 100.0,
            "name_da": "Skyr, 0.2 % fedt", "name_en": "Skyr, 0.2 % fat",
            "product": {"barcode": SKYR, "name": "Vanilla Skyr", "brand": "Synthetic Dairy"},
        })
    );
    let first = api.logged(by_name("skyr snack", "log-1")).await;
    let skyr = &first["portions"][0];
    assert_eq!(
        (
            &skyr["product"]["barcode"],
            &skyr["food_id"],
            &skyr["nova_class"],
            &skyr["nutrients"]["energy_kcal"],
            &skyr["nutrients"]["dietary_fibre"],
        ),
        (
            &json!(SKYR),
            &json!(1693),
            &json!(3),
            &json!({"value": 80.0, "unit": "kcal", "source": "label"}),
            &json!({"value": 0.0, "unit": "g", "source": "food"}),
        )
    );

    api.link(SKYR, 907, "link-2").await;
    let mut later = by_name("Skyr snack", "log-2");
    later["eaten_at"] = json!("2026-10-29T15:00:00+01:00");
    later["overrides"] = json!([{"barcode": SKYR, "grams": 50}]);
    let second = api.logged(later).await;

    let skyr = &second["portions"][0];
    assert_eq!(
        (
            &skyr["food_id"],
            &skyr["grams"],
            &skyr["nutrients"]["energy_kcal"]["value"],
            &skyr["nutrients"]["dietary_fibre"],
        ),
        (
            &json!(907),
            &json!(50.0),
            &json!(40.0),
            &json!({"value": null, "unit": "g", "source": null}),
        )
    );
    assert_eq!(
        api.day("2026-10-29").await["meal_logs"],
        json!([first, second])
    );
    assert_eq!(
        api.meals().await[0]["portions"][0]["name_en"],
        "Chicken, breast, flesh and skin, raw"
    );
}

#[tokio::test]
async fn a_product_whose_linked_food_left_the_food_database_must_be_relinked_before_it_is_logged() {
    let mut api = Api::start().await;
    api.link(SKYR, 3, "link-banana").await;
    api.saved(json!({
        "name": "Skyr snack",
        "portions": [{"barcode": SKYR, "grams": 150}],
        "request_id": "skyr-snack",
    }))
    .await;
    api.reimport("2", second_dataset()).await;

    let relink = |body: &Value, food: &str| {
        assert_eq!(body["code"], "product_needs_relink", "{body}");
        let error = body["error"].as_str().unwrap();
        assert!(
            error.contains(&format!("Product {SKYR} is linked to Food {food}"))
                && error.contains("link_product"),
            "{body}"
        );
    };
    let (status, body) = api.log(product_at(SKYR, "08:00:00", "direct-1")).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    relink(&body, "3 \"Banana, raw\"");
    let mut snack = by_name("Skyr snack", "by-name-1");
    snack["eaten_at"] = json!("2026-10-29T09:00:00+01:00");
    let (status, body) = api.log(snack).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    relink(&body, "3 \"Banana, raw\"");
    assert_eq!(api.day("2026-10-29").await["meal_logs"], json!([]));

    api.link(SKYR, 2, "link-apple").await;
    let logged = api.logged(product_at(SKYR, "10:00:00", "direct-2")).await;
    assert_eq!(
        logged["portions"][0]["name_en"],
        "Apple, raw, all varieties"
    );

    api.reimport(
        "3",
        crate::common::food_workbook::workbook(
            &crate::common::food_workbook::parameters("g/100g"),
            &[
                crate::common::food_workbook::SyntheticFood {
                    id: 2,
                    ..crate::common::food_workbook::rye_bread()
                },
                crate::common::food_workbook::skyr(12.0),
            ],
        ),
    )
    .await;
    let (status, body) = api.log(product_at(SKYR, "11:00:00", "direct-3")).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    relink(&body, "2 \"Apple, raw, all varieties\"");
}

#[tokio::test]
async fn a_meal_whose_food_left_the_food_database_must_be_updated_before_it_is_logged() {
    let mut api = Api::start().await;
    let meal = api
        .saved(json!({
            "name": "Banana skyr",
            "portions": [{"food_id": 3, "grams": 120}, {"food_id": 1693, "grams": 150}],
            "request_id": "banana-skyr-1",
        }))
        .await;
    api.reimport("2", second_dataset()).await;

    let (status, body) = api.log(by_name("Banana skyr", "log-1")).await;

    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["code"], "meal_needs_update");
    assert!(
        body["error"]
            .as_str()
            .unwrap()
            .contains("Food 3 \"Banana, raw\""),
        "{body}"
    );
    assert_eq!(api.day("2026-10-29").await["meal_logs"], json!([]));
    assert_eq!(api.meals().await, std::slice::from_ref(&meal));
    let (status, updated) = api
        .update(
            &meal["id"],
            json!({"portions": [{"food_id": 2, "grams": 150}, {"food_id": 1693, "grams": 150}]}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    let meal_log = api.logged(by_name("Banana skyr", "log-1")).await;
    assert_eq!(
        meal_log["portions"][0]["name_en"],
        "Apple, raw, all varieties"
    );
    assert_eq!(
        meal_log["portions"][1]["nutrients"]["protein"]["value"],
        18.0
    );
    assert_eq!(meal_log["portions"][1]["dataset_version"], "2");

    api.reimport(
        "3",
        crate::common::food_workbook::workbook(
            &crate::common::food_workbook::parameters("g/100g"),
            &[
                crate::common::food_workbook::SyntheticFood {
                    id: 2,
                    ..crate::common::food_workbook::rye_bread()
                },
                crate::common::food_workbook::skyr(12.0),
            ],
        ),
    )
    .await;
    let mut renumbered = by_name("Banana skyr", "log-2");
    renumbered["eaten_at"] = json!("2026-10-29T12:00:00+01:00");
    let (status, body) = api.log(renumbered).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert!(
        body["error"]
            .as_str()
            .unwrap()
            .contains("Food 2 \"Apple, raw, all varieties\""),
        "{body}"
    );
}
