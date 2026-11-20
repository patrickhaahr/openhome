mod common;
#[path = "nutrition_support/meals.rs"]
mod support;

use axum::http::StatusCode;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use support::{Api, SKYR, by_name, protein_bowl};

#[tokio::test]
async fn a_saved_meal_lists_its_ordered_items_by_reference_under_a_case_insensitively_unique_name()
{
    let api = Api::start().await;

    let meal = api.saved(protein_bowl("bowl-1")).await;

    assert_eq!(meal["name"], "Protein bowl");
    assert_eq!(
        meal["portions"],
        json!([
            {
                "food_id": 1693, "barcode": null, "grams": 150.0,
                "name_da": "Skyr, 0.2 % fedt", "name_en": "Skyr, 0.2 % fat", "product": null,
            },
            {
                "food_id": 907, "barcode": null, "grams": 120.0,
                "name_da": "Kylling, bryst, kød og skind, rå",
                "name_en": "Chicken, breast, flesh and skin, raw", "product": null,
            },
        ])
    );
    assert!(meal["portions"][0].get("nutrients").is_none());
    let banana = api
        .saved(json!({
            "name": "banana",
            "portions": [{"food_id": 3, "grams": 120}],
            "request_id": "banana-1",
        }))
        .await;
    assert_eq!(api.meals().await, [banana, meal]);

    let mut shouted = protein_bowl("bowl-2");
    shouted["name"] = json!("PROTEIN BOWL");
    let (status, body) = api.save(shouted).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(api.meals().await.len(), 2);
}

#[tokio::test]
async fn a_retried_save_replays_conflicts_on_changes_and_never_recreates_a_deleted_meal() {
    let api = Api::start().await;
    let meal = api.saved(protein_bowl("bowl-1")).await;

    let mut retry = protein_bowl("bowl-1");
    retry["name"] = json!("Protein bowl");
    let (status, replayed) = api.save(retry).await;
    assert_eq!(status, StatusCode::OK, "{replayed}");
    assert_eq!(replayed, json!({"meal": meal, "replayed": true}));

    let mut changed = protein_bowl("bowl-1");
    changed["portions"][0]["grams"] = json!(200);
    let (status, body) = api.save(changed).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(api.meals().await, std::slice::from_ref(&meal));
    let (status, _) = api
        .update(&meal["id"], json!({"name": "Changed bowl"}))
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, replayed) = api.save(protein_bowl("bowl-1")).await;
    assert_eq!(status, StatusCode::OK, "{replayed}");
    assert_eq!(replayed["meal"], meal);

    assert_eq!(api.delete(&meal["id"]).await, StatusCode::NO_CONTENT);
    let (status, body) = api.save(protein_bowl("bowl-1")).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert!(
        body["error"].as_str().unwrap().contains("deleted"),
        "{body}"
    );
    assert_eq!(api.meals().await, Vec::<Value>::new());

    let mut unknown = protein_bowl("rejected-first");
    unknown["portions"][1]["food_id"] = json!(424_242);
    let (status, body) = api.save(unknown).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    api.saved(protein_bowl("rejected-first")).await;
}

#[tokio::test]
async fn a_legacy_receipt_directs_the_client_to_read_the_meal_first() {
    let api = Api::start().await;
    let meal = api.saved(protein_bowl("bowl-1")).await;
    sqlx::query("UPDATE meal_receipts SET result = NULL WHERE request_id = 'bowl-1'")
        .execute(&api.state.db)
        .await
        .unwrap();

    let (status, body) = api.save(protein_bowl("bowl-1")).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert!(
        body["error"]
            .as_str()
            .unwrap()
            .contains("no original result"),
        "{body}"
    );
    assert_eq!(api.meals().await, [meal]);
}

#[tokio::test]
async fn renaming_a_meal_to_another_meals_name_conflicts_and_a_change_of_case_is_allowed() {
    let api = Api::start().await;
    let bowl = api.saved(protein_bowl("bowl-1")).await;
    let banana = api
        .saved(json!({
            "name": "Banana",
            "portions": [{"food_id": 3, "grams": 120}],
            "request_id": "banana-1",
        }))
        .await;

    let (status, body) = api
        .update(&banana["id"], json!({"name": "protein BOWL"}))
        .await;

    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(api.meals().await, [banana.clone(), bowl.clone()]);
    let (status, renamed) = api
        .update(&bowl["id"], json!({"name": "PROTEIN BOWL"}))
        .await;
    assert_eq!(status, StatusCode::OK, "{renamed}");
    assert_eq!(
        (&renamed["name"], &renamed["portions"]),
        (&json!("PROTEIN BOWL"), &bowl["portions"])
    );
}

#[tokio::test]
async fn invalid_meals_and_logs_by_name_are_refused_without_writing() {
    let api = Api::start().await;
    let long_name = "x".repeat(101);
    for (body, expected) in [
        (json!({"name": "   "}), StatusCode::BAD_REQUEST),
        (json!({"name": long_name}), StatusCode::BAD_REQUEST),
        (json!({"name": "Tab\tbed\u{7}"}), StatusCode::BAD_REQUEST),
        (json!({"portions": []}), StatusCode::BAD_REQUEST),
        (
            json!({"portions": [{"food_id": 1693, "barcode": SKYR, "grams": 10}]}),
            StatusCode::BAD_REQUEST,
        ),
        (
            json!({"portions": [{"food_id": 1693, "grams": 100}, {"food_id": 1693, "grams": 50}]}),
            StatusCode::BAD_REQUEST,
        ),
        (
            json!({"portions": [{"food_id": 1693, "grams": 0}]}),
            StatusCode::BAD_REQUEST,
        ),
        (
            json!({"portions": [{"food_id": 1693, "grams": 10, "estimates": {"vitamin_d": 1}}]}),
            StatusCode::BAD_REQUEST,
        ),
        (json!({"request_id": " "}), StatusCode::BAD_REQUEST),
        (
            json!({"portions": [{"food_id": 424_242, "grams": 10}]}),
            StatusCode::NOT_FOUND,
        ),
    ] {
        let mut request = protein_bowl("bowl-1");
        for (key, value) in body.as_object().unwrap() {
            request[key] = value.clone();
        }
        let (status, response) = api.save(request.clone()).await;
        assert_eq!(status, expected, "{request}: {response}");
    }
    assert_eq!(api.meals().await, Vec::<Value>::new());

    let meal = api.saved(protein_bowl("bowl-1")).await;
    for edit in [
        json!({"name": null}),
        json!({"portions": null}),
        json!({"name": ""}),
    ] {
        let (status, body) = api.update(&meal["id"], edit.clone()).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{edit}: {body}");
    }
    assert_eq!(api.meals().await, [meal]);

    let both = json!({"portions": [{"food_id": 1693, "grams": 10}]});
    for (change, expected) in [
        (both, StatusCode::BAD_REQUEST),
        (json!({"meal": null}), StatusCode::BAD_REQUEST),
        (json!({"meal": "Lasagne"}), StatusCode::NOT_FOUND),
        (
            json!({"overrides": [{"food_id": 3, "grams": 10}]}),
            StatusCode::BAD_REQUEST,
        ),
        (
            json!({"overrides": [{"food_id": 1693, "grams": 10}, {"food_id": 1693, "grams": 20}]}),
            StatusCode::BAD_REQUEST,
        ),
        (
            json!({"overrides": [{"food_id": 1693, "grams": 0}]}),
            StatusCode::BAD_REQUEST,
        ),
        (
            json!({"overrides": [{"food_id": 1693, "grams": 10, "estimates": {}}]}),
            StatusCode::BAD_REQUEST,
        ),
    ] {
        let mut request = by_name("Protein bowl", "log-1");
        for (key, value) in change.as_object().unwrap() {
            request[key] = value.clone();
        }
        let (status, body) = api.log(request.clone()).await;
        assert_eq!(status, expected, "{request}: {body}");
    }
    let overrides_without_meal = json!({
        "eaten_at": "2026-10-29T08:15:00+01:00",
        "portions": [{"food_id": 1693, "grams": 10}],
        "overrides": [{"food_id": 1693, "grams": 20}],
        "request_id": "log-1",
    });
    let (status, body) = api.log(overrides_without_meal).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(api.day("2026-10-29").await["meal_logs"], json!([]));
    api.logged(by_name("Protein bowl", "log-1")).await;
}

#[tokio::test]
async fn while_the_food_database_is_unavailable_meals_stay_listable_renamable_and_deletable() {
    let api = Api::start().await;
    let meal = api.saved(protein_bowl("bowl-1")).await;
    let api = api.importing().await;

    for (status, body) in [
        api.save(json!({
            "name": "Chicken",
            "portions": [{"food_id": 907, "grams": 100}],
            "request_id": "chicken-1",
        }))
        .await,
        api.update(
            &meal["id"],
            json!({"portions": [{"food_id": 907, "grams": 100}]}),
        )
        .await,
        api.log(by_name("Protein bowl", "log-1")).await,
    ] {
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
        assert_eq!(
            (&body["code"], &body["state"]),
            (&json!("food_database_unavailable"), &json!("importing"))
        );
    }
    assert_eq!(api.meals().await, std::slice::from_ref(&meal));
    let (status, renamed) = api.update(&meal["id"], json!({"name": "Bowl"})).await;
    assert_eq!(status, StatusCode::OK, "{renamed}");
    assert_eq!(renamed["name"], "Bowl");
    assert_eq!(api.delete(&meal["id"]).await, StatusCode::NO_CONTENT);
}
