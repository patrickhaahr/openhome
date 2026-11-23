use crate::support::{Api, breakfast, creatine, fish_oil};
use axum::http::StatusCode;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

#[tokio::test]
async fn a_supplement_dose_counts_its_per_dose_amounts_times_the_doses_taken() {
    let api = Api::start().await;
    let oil = api.added(fish_oil("oil")).await;
    let creatine = api.added(creatine("creatine")).await;

    let log = api
        .logged(breakfast(
            "breakfast",
            &json!([
                {"supplement_id": oil["id"], "doses": 2},
                {"supplement_id": creatine["id"], "doses": 1},
            ]),
        ))
        .await;

    assert_eq!(
        log["supplement_doses"],
        json!([
            {
                "supplement_id": oil["id"], "name": "Omega-3 fish oil", "dose_unit": "capsule",
                "doses": 2.0,
                "amounts": [
                    {"key": "epa", "form": null, "value": 0.36, "unit": "g"},
                    {"key": "dha", "form": null, "value": 0.24, "unit": "g"},
                    {"key": "vitamin_d", "form": null, "value": 10.0, "unit": "µg"},
                    {"key": "energy_kcal", "form": null, "value": 18.0, "unit": "kcal"},
                    {"key": "fat", "form": null, "value": 2.0, "unit": "g"},
                    {"key": "energy_kj", "form": null, "value": 75.312, "unit": "kJ"},
                ],
                "compounds": [],
            },
            {
                "supplement_id": creatine["id"], "name": "Creatine", "dose_unit": "scoop",
                "doses": 1.0, "amounts": [],
                "compounds": [{"name": "Creatine monohydrate", "value": 3.0, "unit": "g"}],
            },
        ])
    );
    assert_eq!(
        (
            &log["totals"]["energy_kcal"],
            &log["totals"]["fat"],
            &log["totals"]["protein"]
        ),
        (
            &json!({
                "value": 318.0, "unit": "kcal", "unknown_portions": 0,
                "food": 300.0, "supplements": 18.0,
            }),
            &json!({
                "value": 16.0, "unit": "g", "unknown_portions": 0,
                "food": 14.0, "supplements": 2.0,
            }),
            &json!({
                "value": 42.0, "unit": "g", "unknown_portions": 0,
                "food": 42.0, "supplements": null,
            }),
        )
    );
}

#[tokio::test]
async fn the_day_splits_every_total_into_food_and_supplement_shares_and_lists_compounds_taken() {
    let api = Api::start().await;
    let oil = api.added(fish_oil("oil")).await;
    let creatine = api.added(creatine("creatine")).await;
    api.logged(breakfast(
        "breakfast",
        &json!([
            {"supplement_id": oil["id"], "doses": 2},
            {"supplement_id": creatine["id"], "doses": 1},
        ]),
    ))
    .await;
    api.logged(json!({
        "eaten_at": "2026-10-29T21:00:00+01:00",
        "supplement_doses": [
            {"supplement_id": oil["id"], "doses": 1},
            {"supplement_id": creatine["id"], "doses": 1},
        ],
        "request_id": "evening",
    }))
    .await;

    let day = api.day("2026-10-29").await;

    assert_eq!(
        api.nutrient("2026-10-29", "epa").await,
        json!({
            "key": "epa", "name_da": "C20:5,n-3", "name_en": "C20:5,n-3",
            "group": {"name_da": "Flerumættede fedtsyrer", "name_en": "Polyunsaturated fatty acids"},
            "unit": "g", "value": 0.56, "unknown_portions": 0, "food": 0.02, "supplements": 0.54,
        })
    );
    let vitamin_d = api.nutrient("2026-10-29", "vitamin_d").await;
    assert_eq!(
        (
            &vitamin_d["value"],
            &vitamin_d["food"],
            &vitamin_d["supplements"]
        ),
        (&json!(15.0), &Value::Null, &json!(15.0))
    );
    assert_eq!(vitamin_d["unknown_portions"], 1);
    let dha = api.nutrient("2026-10-29", "dha").await;
    assert_eq!(
        (
            &dha["name_en"],
            &dha["unit"],
            &dha["value"],
            &dha["supplements"]
        ),
        (&Value::Null, &json!("g"), &json!(0.36), &json!(0.36))
    );
    assert_eq!(
        day["totals"]["energy_kcal"],
        json!({
            "value": 327.0, "unit": "kcal", "unknown_portions": 0,
            "food": 300.0, "supplements": 27.0,
        })
    );
    assert_eq!(
        day["compounds"],
        json!([{"name": "Creatine monohydrate", "value": 6.0, "unit": "g"}])
    );
    assert_eq!(
        day["supplement_doses"],
        json!([
            {
                "supplement_id": oil["id"], "name": "Omega-3 fish oil",
                "dose_unit": "capsule", "doses": 3.0,
            },
            {
                "supplement_id": creatine["id"], "name": "Creatine",
                "dose_unit": "scoop", "doses": 2.0,
            },
        ])
    );
    assert_eq!(day["nova"]["unclassified_portions"], 1);
}

#[tokio::test]
async fn the_day_gives_the_supplement_share_of_each_upper_limit_form() {
    let api = Api::start().await;
    let multivitamin = api
        .added(json!({
            "name": "Multivitamin",
            "dose_unit": "tablet",
            "amounts": [
                {"key": "vitamin_a", "value": 300, "unit": "µg", "form": "retinol"},
                {"key": "vitamin_a", "value": 100, "unit": "µg", "form": "beta_carotene"},
                {"key": "folate", "value": 200, "unit": "µg", "form": "folic_acid"},
                {"key": "magnesium", "value": 100, "unit": "mg"},
            ],
            "request_id": "multi",
        }))
        .await;

    api.logged(breakfast(
        "breakfast",
        &json!([{"supplement_id": multivitamin["id"], "doses": 2}]),
    ))
    .await;

    let vitamin_a = api.nutrient("2026-10-29", "vitamin_a").await;
    assert_eq!(
        (&vitamin_a["supplements"], &vitamin_a["supplement_forms"]),
        (
            &json!(800.0),
            &json!([
                {"form": "beta_carotene", "value": 200.0},
                {"form": "retinol", "value": 600.0},
            ])
        )
    );
    let folate = api.nutrient("2026-10-29", "folate").await;
    assert_eq!(
        folate["supplement_forms"],
        json!([{"form": "folic_acid", "value": 400.0}])
    );
    let magnesium = api.nutrient("2026-10-29", "magnesium").await;
    assert_eq!(
        (&magnesium["supplements"], magnesium.get("supplement_forms")),
        (&json!(200.0), None)
    );
}

#[tokio::test]
async fn a_logged_dose_keeps_its_snapshot_after_the_supplement_is_edited_or_deleted() {
    let api = Api::start().await;
    let oil = api.added(fish_oil("oil")).await;
    let creatine = api.added(creatine("creatine")).await;
    let log = api
        .logged(breakfast(
            "breakfast",
            &json!([
                {"supplement_id": oil["id"], "doses": 2},
                {"supplement_id": creatine["id"], "doses": 1},
            ]),
        ))
        .await;
    let before = api.day("2026-10-29").await;

    let (status, body) = api
        .update(
            &oil["id"],
            json!({
                "name": "Stronger oil",
                "amounts": [{"key": "epa", "value": 500, "unit": "mg"}],
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body) = api.delete(&creatine["id"]).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");

    assert_eq!(
        api.meal_logs("2026-10-29").await,
        std::slice::from_ref(&log)
    );
    assert_eq!(api.day("2026-10-29").await, before);
    let next = api
        .logged(json!({
            "eaten_at": "2026-10-29T12:00:00+01:00",
            "supplement_doses": [{"supplement_id": oil["id"], "doses": 1}],
            "request_id": "lunch",
        }))
        .await;
    assert_eq!(
        (
            &next["supplement_doses"][0]["name"],
            &next["supplement_doses"][0]["amounts"]
        ),
        (
            &json!("Stronger oil"),
            &json!([{"key": "epa", "form": null, "value": 0.5, "unit": "g"}])
        )
    );
}

#[tokio::test]
async fn supplement_doses_are_checked_retried_and_replaced_like_portions() {
    let api = Api::start().await;
    let oil = api.added(fish_oil("oil")).await;
    let creatine = api.added(creatine("creatine")).await;
    let doses = json!([{"supplement_id": oil["id"], "doses": 2}]);
    let log = api.logged(breakfast("breakfast", &doses)).await;

    let (status, replayed) = api.log(breakfast("breakfast", &doses)).await;
    assert_eq!(status, StatusCode::OK, "{replayed}");
    assert_eq!(replayed["meal_log"], log);
    let (status, body) = api
        .log(breakfast(
            "breakfast",
            &json!([{"supplement_id": oil["id"], "doses": 3}]),
        ))
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");

    for (doses, expected) in [
        (
            json!([{"supplement_id": "missing", "doses": 1}]),
            StatusCode::NOT_FOUND,
        ),
        (
            json!([{"supplement_id": oil["id"], "doses": 0}]),
            StatusCode::BAD_REQUEST,
        ),
        (
            json!([{"supplement_id": oil["id"], "doses": 101}]),
            StatusCode::BAD_REQUEST,
        ),
        (
            json!([
                {"supplement_id": oil["id"], "doses": 1},
                {"supplement_id": oil["id"], "doses": 1},
            ]),
            StatusCode::BAD_REQUEST,
        ),
    ] {
        let (status, body) = api.log(breakfast("rejected", &doses)).await;
        assert_eq!(status, expected, "{doses}: {body}");
    }
    let (status, body) = api
        .log(json!({
            "eaten_at": "2026-10-29T08:15:00+01:00",
            "portions": [],
            "supplement_doses": [],
            "request_id": "nothing",
        }))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");

    let (status, updated) = api
        .update_log(
            &log["id"],
            json!({"supplement_doses": [{"supplement_id": creatine["id"], "doses": 2}]}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    assert_eq!(updated["portions"], log["portions"]);
    assert_eq!(
        updated["supplement_doses"][0]["compounds"],
        json!([{"name": "Creatine monohydrate", "value": 6.0, "unit": "g"}])
    );
    let (status, updated) = api.update_log(&log["id"], json!({"portions": []})).await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    assert_eq!(updated["portions"], json!([]));
    let (status, body) = api
        .update_log(&log["id"], json!({"supplement_doses": []}))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
}

#[tokio::test]
async fn supplement_doses_are_logged_and_listed_while_the_food_database_imports() {
    let api = Api::start().await.importing().await;
    let oil = api.added(fish_oil("oil")).await;

    let log = api
        .logged(json!({
            "eaten_at": "2026-10-29T08:15:00+01:00",
            "supplement_doses": [{"supplement_id": oil["id"], "doses": 1}],
            "request_id": "stack",
        }))
        .await;

    assert_eq!(api.meal_logs("2026-10-29").await, [log]);
}
