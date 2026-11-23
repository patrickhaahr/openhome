use crate::support::{Api, creatine, fish_oil};
use axum::http::StatusCode;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

#[tokio::test]
async fn a_supplement_keeps_per_dose_amounts_in_parameter_units_under_a_unique_name() {
    let api = Api::start().await;

    let oil = api.added(fish_oil("oil-1")).await;

    assert_eq!(oil["name"], "Omega-3 fish oil");
    assert_eq!(oil["dose_unit"], "capsule");
    assert_eq!(
        oil["amounts"],
        json!([
            {"key": "epa", "form": null, "value": 0.18, "unit": "g"},
            {"key": "dha", "form": null, "value": 0.12, "unit": "g"},
            {"key": "vitamin_d", "form": null, "value": 5.0, "unit": "µg"},
            {"key": "energy_kcal", "form": null, "value": 9.0, "unit": "kcal"},
            {"key": "fat", "form": null, "value": 1.0, "unit": "g"},
            {"key": "energy_kj", "form": null, "value": 37.656, "unit": "kJ"},
        ])
    );
    assert_eq!(oil["compounds"], json!([]));
    let creatine = api.added(creatine("creatine-1")).await;
    assert_eq!(
        creatine["compounds"],
        json!([{"name": "Creatine monohydrate", "value": 3.0, "unit": "g"}])
    );
    assert_eq!(api.supplements().await, [creatine, oil.clone()]);

    let mut shouted = fish_oil("oil-2");
    shouted["name"] = json!("OMEGA-3 FISH OIL");
    let (status, body) = api.add(shouted).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(api.supplements().await.len(), 2);
}

#[tokio::test]
async fn supplement_only_upper_limit_forms_are_named_on_their_amounts() {
    let api = Api::start().await;

    let multivitamin = api
        .added(json!({
            "name": "Multivitamin",
            "dose_unit": "tablet",
            "amounts": [
                {"key": "vitamin_a", "value": 400, "unit": "µg", "form": "retinol"},
                {"key": "vitamin_a", "value": 400, "unit": "µg RE", "form": "beta_carotene"},
                {"key": "folate", "value": 0.2, "unit": "mg", "form": "folic_acid"},
                {"key": "niacin_equivalent", "value": 16, "unit": "mg", "form": "nicotinamide"},
                {"key": "magnesium", "value": 100, "unit": "mg"},
            ],
            "request_id": "multi-1",
        }))
        .await;

    assert_eq!(
        multivitamin["amounts"],
        json!([
            {"key": "vitamin_a", "form": "retinol", "value": 400.0, "unit": "µg RE"},
            {"key": "vitamin_a", "form": "beta_carotene", "value": 400.0, "unit": "µg RE"},
            {"key": "folate", "form": "folic_acid", "value": 200.0, "unit": "µg"},
            {"key": "niacin_equivalent", "form": "nicotinamide", "value": 16.0, "unit": "mg NE"},
            {"key": "magnesium", "form": null, "value": 100.0, "unit": "mg"},
        ])
    );
}

#[tokio::test]
async fn a_supplement_amount_needs_a_parameter_key_its_unit_and_a_form_where_limits_differ() {
    let api = Api::start().await;
    let refused = [
        json!({"key": "vitamin_q", "value": 1, "unit": "mg"}),
        json!({"key": "waste", "value": 1, "unit": "%"}),
        json!({"key": "vitamin_d", "value": 1, "unit": "kcal"}),
        json!({"key": "vitamin_d", "value": -1, "unit": "µg"}),
        json!({"key": "folate", "value": 400, "unit": "µg"}),
        json!({"key": "folate", "value": 400, "unit": "µg", "form": "retinol"}),
        json!({"key": "zinc", "value": 10, "unit": "mg", "form": "folic_acid"}),
    ];

    for amount in refused {
        let (status, body) = api
            .add(json!({
                "name": "Refused",
                "dose_unit": "tablet",
                "amounts": [amount],
                "request_id": "refused",
            }))
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{amount}: {body}");
    }
    let (_, body) = api
        .add(json!({
            "name": "Folate",
            "dose_unit": "tablet",
            "amounts": [{"key": "folate", "value": 400, "unit": "µg"}],
            "request_id": "folate",
        }))
        .await;
    assert!(
        body["error"].as_str().unwrap().contains("folic_acid"),
        "{body}"
    );
    for supplement in [
        json!({"name": "Twice", "dose_unit": "tablet", "request_id": "twice", "amounts": [
            {"key": "zinc", "value": 10, "unit": "mg"}, {"key": "zinc", "value": 5, "unit": "mg"},
        ]}),
        json!({"name": "Empty", "dose_unit": "tablet", "request_id": "empty"}),
        json!({"name": "Pill", "dose_unit": "pill", "request_id": "pill", "amounts": [
            {"key": "zinc", "value": 10, "unit": "mg"},
        ]}),
        json!({"name": "D", "dose_unit": "tablet", "request_id": "d", "compounds": [
            {"name": "Vitamin D", "value": 25, "unit": "µg"},
        ]}),
        json!({"name": "Probiotic", "dose_unit": "capsule", "request_id": "cfu", "compounds": [
            {"name": "Lactobacillus", "value": 10, "unit": "billion"},
        ]}),
    ] {
        let (status, body) = api.add(supplement.clone()).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{supplement}: {body}");
    }
    assert_eq!(api.supplements().await, Vec::<Value>::new());
}

#[tokio::test]
async fn a_retried_addition_replays_conflicts_on_changes_and_never_recreates_a_deleted_supplement()
{
    let api = Api::start().await;
    let oil = api.added(fish_oil("oil-1")).await;

    let (status, replayed) = api.add(fish_oil("oil-1")).await;
    assert_eq!(status, StatusCode::OK, "{replayed}");
    assert_eq!(replayed, json!({"supplement": oil, "replayed": true}));

    let mut changed = fish_oil("oil-1");
    changed["amounts"][0]["value"] = json!(200);
    let (status, body) = api.add(changed).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");

    let (status, _) = api
        .update(
            &oil["id"],
            json!({"name": "Changed oil", "dose_unit": "ml"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, replayed) = api.add(fish_oil("oil-1")).await;
    assert_eq!(status, StatusCode::OK, "{replayed}");
    assert_eq!(replayed["supplement"], oil);

    let (status, _) = api.delete(&oil["id"]).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, body) = api.add(fish_oil("oil-1")).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert!(
        body["error"].as_str().unwrap().contains("deleted"),
        "{body}"
    );
    assert_eq!(api.supplements().await, Vec::<Value>::new());
}

#[tokio::test]
async fn a_legacy_receipt_directs_the_client_to_read_the_supplement_first() {
    let api = Api::start().await;
    let oil = api.added(fish_oil("oil-1")).await;
    sqlx::query("UPDATE supplement_receipts SET result = NULL WHERE request_id = 'oil-1'")
        .execute(&api.state.db)
        .await
        .unwrap();

    let (status, body) = api.add(fish_oil("oil-1")).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert!(
        body["error"]
            .as_str()
            .unwrap()
            .contains("no original result"),
        "{body}"
    );
    assert_eq!(api.supplements().await, [oil]);
}

#[tokio::test]
async fn updating_a_supplement_renames_it_or_replaces_its_amounts_together() {
    let api = Api::start().await;
    let oil = api.added(fish_oil("oil-1")).await;
    let creatine = api.added(creatine("creatine-1")).await;

    let (status, updated) = api
        .update(
            &oil["id"],
            json!({
                "name": "Fish oil",
                "dose_unit": "ml",
                "amounts": [{"key": "epa", "value": 0.3, "unit": "g"}],
            }),
        )
        .await;

    assert_eq!(status, StatusCode::OK, "{updated}");
    assert_eq!(
        updated,
        json!({
            "id": oil["id"], "name": "Fish oil", "dose_unit": "ml",
            "amounts": [{"key": "epa", "form": null, "value": 0.3, "unit": "g"}],
            "compounds": [],
        })
    );
    let (status, body) = api.update(&oil["id"], json!({"name": "creatine"})).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    let (status, body) = api.update(&creatine["id"], json!({"compounds": []})).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(api.supplements().await, [creatine, updated]);
    let (status, body) = api
        .update(&json!("missing"), json!({"name": "Ghost"}))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
}
