mod common;
#[path = "nutrition_support/supplements.rs"]
mod support;

use axum::http::StatusCode;
use pretty_assertions::assert_eq;
use serde_json::json;
use support::{Api, creatine, fish_oil};

#[tokio::test]
async fn a_meal_takes_supplement_doses_by_reference_and_logs_them_with_dose_overrides() {
    let api = Api::start().await;
    let oil = api.added(fish_oil("oil")).await;
    let creatine = api.added(creatine("creatine")).await;
    let (status, saved) = api
        .save_meal(json!({
            "name": "Morning stack",
            "portions": [{"food_id": 1693, "grams": 150}],
            "supplement_doses": [
                {"supplement_id": oil["id"], "doses": 2},
                {"supplement_id": creatine["id"], "doses": 1},
            ],
            "request_id": "stack",
        }))
        .await;
    assert_eq!(status, StatusCode::CREATED, "{saved}");
    let meal = &saved["meal"];
    assert_eq!(
        meal["supplement_doses"],
        json!([
            {
                "supplement_id": oil["id"], "name": "Omega-3 fish oil",
                "dose_unit": "capsule", "doses": 2.0,
            },
            {
                "supplement_id": creatine["id"], "name": "Creatine",
                "dose_unit": "scoop", "doses": 1.0,
            },
        ])
    );
    api.update(
        &oil["id"],
        json!({"amounts": [{"key": "epa", "value": 250, "unit": "mg"}]}),
    )
    .await;

    let log = api
        .logged(json!({
            "eaten_at": "2026-10-29T07:30:00+01:00",
            "meal": "morning STACK",
            "overrides": [{"supplement_id": oil["id"], "doses": 1}],
            "request_id": "morning",
        }))
        .await;

    assert_eq!(log["meal_name"], "Morning stack");
    assert_eq!(log["portions"][0]["grams"], 150.0);
    assert_eq!(
        log["supplement_doses"]
            .as_array()
            .unwrap()
            .iter()
            .map(|dose| (dose["name"].clone(), dose["doses"].clone()))
            .collect::<Vec<_>>(),
        [
            (json!("Omega-3 fish oil"), json!(1.0)),
            (json!("Creatine"), json!(1.0)),
        ]
    );
    assert_eq!(
        log["supplement_doses"][0]["amounts"],
        json!([{"key": "epa", "form": null, "value": 0.25, "unit": "g"}])
    );
    let (status, body) = api
        .log(json!({
            "eaten_at": "2026-10-29T07:30:00+01:00",
            "meal": "Morning stack",
            "overrides": [{"supplement_id": "missing", "doses": 1}],
            "request_id": "bad-override",
        }))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");

    let (status, body) = api.delete(&creatine["id"]).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert!(
        body["error"].as_str().unwrap().contains("Morning stack"),
        "{body}"
    );
    let (status, updated) = api
        .update_meal(
            &meal["id"],
            json!({"supplement_doses": [{"supplement_id": oil["id"], "doses": 2}]}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    assert_eq!(updated["portions"], meal["portions"]);
    assert_eq!(updated["supplement_doses"].as_array().unwrap().len(), 1);
    let (status, body) = api.delete(&creatine["id"]).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    assert_eq!(api.delete_meal(&meal["id"]).await, StatusCode::NO_CONTENT);
    let (status, body) = api.delete(&oil["id"]).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
}

#[tokio::test]
async fn a_meal_of_only_supplement_doses_is_saved_and_needs_at_least_one_item() {
    let api = Api::start().await;
    let oil = api.added(fish_oil("oil")).await;

    let (status, saved) = api
        .save_meal(json!({
            "name": "Evening stack",
            "supplement_doses": [{"supplement_id": oil["id"], "doses": 1}],
            "request_id": "evening",
        }))
        .await;

    assert_eq!(status, StatusCode::CREATED, "{saved}");
    assert_eq!(saved["meal"]["portions"], json!([]));
    let (status, body) = api
        .update_meal(&saved["meal"]["id"], json!({"supplement_doses": []}))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    for (doses, expected) in [
        (
            json!([{"supplement_id": "missing", "doses": 1}]),
            StatusCode::NOT_FOUND,
        ),
        (json!([]), StatusCode::BAD_REQUEST),
    ] {
        let (status, body) = api
            .save_meal(json!({
                "name": "Rejected",
                "supplement_doses": doses,
                "request_id": "rejected",
            }))
            .await;
        assert_eq!(status, expected, "{body}");
    }
}
