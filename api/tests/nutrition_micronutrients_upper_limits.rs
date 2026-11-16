//! EFSA Upper Limits checked against the Nutrition Day and the trailing 7-day average, from a real
//! `app.db` and a real `food.db` imported from a synthetic DTU-shaped workbook served by `wiremock`.

mod common;
#[path = "nutrition_support/micronutrients.rs"]
mod support;

use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use support::{Api, coverage, limit_flags, limits, nutrient, woman};

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
    // Three days of 10 g of Salmon (1 µg vitamin D), then one 150 µg capsule alone.
    for day in ["2026-03-29", "2026-03-30", "2026-03-31"] {
        api.eat(day, &[(1002, 10.0)], &[]).await;
    }
    api.eat("2026-04-01", &[], &[(&strong, 1.0)]).await;

    // Above the 100 µg Upper Limit that day, but the average of the 4 logged days is 38.25 µg.
    let vitamin_d = api.nutrient("2026-04-01", "vitamin_d").await;
    assert_eq!(
        limits(&vitamin_d),
        json!([["total", 100.0, 150.0, 38.25, 4, "caution"]])
    );
    assert_eq!(vitamin_d["upper_limit_status"], "caution");

    // The next day logs nothing: no caution, and it does not count toward the average as zero.
    let vitamin_d = api.nutrient("2026-04-02", "vitamin_d").await;
    assert_eq!(
        limits(&vitamin_d),
        json!([["total", 100.0, null, 38.25, 4, "none"]])
    );

    // Two more 150 µg days average 150 µg over the 7 days to 2026-04-08, but two logged days
    // are too few to call it a habit.
    api.eat("2026-04-05", &[], &[(&strong, 1.0)]).await;
    api.eat("2026-04-07", &[], &[(&strong, 1.0)]).await;
    let vitamin_d = api.nutrient("2026-04-08", "vitamin_d").await;
    assert_eq!(
        limits(&vitamin_d),
        json!([["total", 100.0, null, 150.0, 2, "none"]])
    );

    // A third, with nothing logged between them, averages 150 µg over 3 logged days because
    // untracked days are left out: cut down, even on a day without a dose.
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

    // A day above the limit within a habitual excess says cut down rather than caution: its
    // window reaches back to 2026-04-01.
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

    // The trailing average is that one day, above the limit, yet one day is not a habit.
    let vitamin_d = api.nutrient("2026-04-08", "vitamin_d").await;
    assert_eq!(
        limits(&vitamin_d),
        json!([["total", 100.0, 150.0, 150.0, 1, "caution"]])
    );
    assert_eq!(vitamin_d["upper_limit_status"], "caution");

    // The summary judges the day the same way: a day above, no cut down.
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
    // 100 g of Pumpkin seeds (600 mg magnesium, 10 mg NE niacin) and 400 g of Wholemeal bread
    // (1,200 µg folate): far above those Upper Limits, but they apply to supplements only.
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

    // The next day adds a magnesium, a folic acid and a nicotinic acid supplement.
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
    // Each niacin form has its own limit: nicotinamide stays clear.
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
    // Vitamin D has a limit the day stays under.
    assert_eq!(nutrient(&panel, "vitamin_d")["upper_limit_status"], "none");
}
