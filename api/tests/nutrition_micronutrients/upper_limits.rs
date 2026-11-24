use crate::support::{Api, coverage, limit_flags, limits, nutrient, woman};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

#[tokio::test]
async fn a_day_above_an_upper_limit_is_a_caution_and_a_seven_day_average_above_it_says_cut_down() {
    let api = Api::start().await;
    woman(&api).await;
    let strong = api
        .supplement(
            "Vitamin D 150",
            json!({"amounts": [{"key": "vitamin_d", "value": 150, "unit": "µg"}]}),
        )
        .await;
    for day in ["2026-03-29", "2026-03-30", "2026-03-31"] {
        api.eat(day, &[(1002, 10.0)], &[]).await;
    }
    api.eat("2026-04-01", &[], &[(&strong, 1.0)]).await;

    let vitamin_d = api.nutrient("2026-04-01", "vitamin_d").await;
    assert_eq!(
        limits(&vitamin_d),
        json!([["total", 100.0, 150.0, 38.25, 4, "caution"]])
    );
    assert_eq!(vitamin_d["upper_limit_status"], "caution");

    let vitamin_d = api.nutrient("2026-04-02", "vitamin_d").await;
    assert_eq!(
        limits(&vitamin_d),
        json!([["total", 100.0, null, 38.25, 4, "none"]])
    );

    api.eat("2026-04-05", &[], &[(&strong, 1.0)]).await;
    api.eat("2026-04-07", &[], &[(&strong, 1.0)]).await;
    let vitamin_d = api.nutrient("2026-04-08", "vitamin_d").await;
    assert_eq!(
        limits(&vitamin_d),
        json!([["total", 100.0, null, 150.0, 2, "none"]])
    );

    api.eat("2026-04-06", &[], &[(&strong, 1.0)]).await;
    let panel = api.panel("2026-04-08").await;
    assert_eq!(
        panel["trailing_window"],
        json!({"from": "2026-04-02", "to": "2026-04-08", "logged_days": 3})
    );
    let vitamin_d = nutrient(&panel, "vitamin_d");
    assert_eq!(
        limits(&vitamin_d),
        json!([["total", 100.0, null, 150.0, 3, "cut_down"]])
    );
    assert_eq!(vitamin_d["upper_limit_status"], "cut_down");

    let vitamin_d = api.nutrient("2026-04-07", "vitamin_d").await;
    assert_eq!(
        limits(&vitamin_d),
        json!([["total", 100.0, 150.0, 150.0, 4, "cut_down"]])
    );
}

#[tokio::test]
async fn a_single_logged_day_above_an_upper_limit_is_only_a_caution() {
    let api = Api::start().await;
    woman(&api).await;
    let strong = api
        .supplement(
            "Vitamin D 150",
            json!({"amounts": [{"key": "vitamin_d", "value": 150, "unit": "µg"}]}),
        )
        .await;
    api.eat("2026-04-08", &[], &[(&strong, 1.0)]).await;

    let vitamin_d = api.nutrient("2026-04-08", "vitamin_d").await;
    assert_eq!(
        limits(&vitamin_d),
        json!([["total", 100.0, 150.0, 150.0, 1, "caution"]])
    );
    assert_eq!(vitamin_d["upper_limit_status"], "caution");

    let week = api.summary("2026-04-06", "2026-04-12", "week").await;
    assert_eq!(
        limit_flags(&week["periods"][0]["micronutrients"], "vitamin_d"),
        json!([1, false])
    );
}

#[tokio::test]
async fn supplement_only_upper_limits_ignore_food_sources() {
    let api = Api::start().await;
    woman(&api).await;
    api.eat("2026-04-08", &[(1003, 100.0), (1005, 400.0)], &[])
        .await;

    let panel = api.panel("2026-04-08").await;
    let magnesium = nutrient(&panel, "magnesium");
    assert_eq!(magnesium["amount"], 600.0);
    assert_eq!(
        limits(&magnesium),
        json!([["supplements", 250.0, 0.0, 0.0, 1, "none"]])
    );
    let folate = nutrient(&panel, "folate");
    assert_eq!(folate["amount"], 1200.0);
    assert_eq!(
        limits(&folate),
        json!([["supplemental_folate", 1000.0, 0.0, 0.0, 1, "none"]])
    );
    assert_eq!(
        limits(&nutrient(&panel, "niacin_equivalent")),
        json!([
            ["nicotinic_acid", 10.0, 0.0, 0.0, 1, "none"],
            ["nicotinamide", 900.0, 0.0, 0.0, 1, "none"],
        ])
    );

    let magnesium = api
        .supplement(
            "Magnesium",
            json!({"amounts": [{"key": "magnesium", "value": 300, "unit": "mg"}]}),
        )
        .await;
    let folic_acid = api
        .supplement(
            "Folic acid",
            json!({"amounts": [
                {"key": "folate", "form": "folic_acid", "value": 1100, "unit": "µg"},
            ]}),
        )
        .await;
    let niacin = api
        .supplement(
            "Niacin",
            json!({"amounts": [
                {"key": "niacin_equivalent", "form": "nicotinic_acid", "value": 20, "unit": "mg"},
            ]}),
        )
        .await;
    api.eat(
        "2026-04-09",
        &[(1003, 100.0)],
        &[(&magnesium, 1.0), (&folic_acid, 1.0), (&niacin, 1.0)],
    )
    .await;

    let panel = api.panel("2026-04-09").await;
    let magnesium = nutrient(&panel, "magnesium");
    assert_eq!(coverage(&magnesium), json!([900.0, 600.0, 300.0, 300.0]));
    assert_eq!(
        limits(&magnesium),
        json!([["supplements", 250.0, 300.0, 150.0, 2, "caution"]])
    );
    assert_eq!(
        limits(&nutrient(&panel, "folate")),
        json!([["supplemental_folate", 1000.0, 1100.0, 550.0, 2, "caution"]])
    );
    let niacin = nutrient(&panel, "niacin_equivalent");
    assert_eq!(
        limits(&niacin),
        json!([
            ["nicotinic_acid", 10.0, 20.0, 10.0, 2, "caution"],
            ["nicotinamide", 900.0, 0.0, 0.0, 2, "none"],
        ])
    );
    assert_eq!(niacin["upper_limit_status"], "caution");
}

#[tokio::test]
async fn nutrients_without_an_upper_limit_are_never_flagged() {
    let api = Api::start().await;
    woman(&api).await;
    let vitamin_c = api
        .supplement(
            "Vitamin C",
            json!({"amounts": [{"key": "vitamin_c", "value": 1000, "unit": "mg"}]}),
        )
        .await;
    for day in ["2026-04-06", "2026-04-07", "2026-04-08"] {
        api.eat(day, &[(1002, 100.0)], &[(&vitamin_c, 2.0)]).await;
    }

    let panel = api.panel("2026-04-08").await;
    let vitamin_c = nutrient(&panel, "vitamin_c");
    assert_eq!(vitamin_c["percent_of_recommended"], 2105.3);
    assert_eq!(vitamin_c["upper_limits"], json!([]));
    assert_eq!(vitamin_c["upper_limit_status"], Value::Null);
    assert_eq!(nutrient(&panel, "vitamin_d")["upper_limit_status"], "none");
}
