mod common;
#[path = "nutrition_support/micronutrients.rs"]
mod support;

use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use support::{Api, limit_flags, nutrient, period_coverage, woman};

#[tokio::test]
async fn summary_coverage_averages_the_recommended_intake_over_logged_days_only() {
    let api = Api::start().await;
    woman(&api).await;
    let vitamin_d = api
        .supplement(
            "Vitamin D3",
            json!({"amounts": [{"key": "vitamin_d", "value": 20, "unit": "µg"},
                {"key": "protein", "value": 12, "unit": "g"}]}),
        )
        .await;
    api.eat("2026-04-06", &[(1001, 200.0)], &[]).await;
    api.eat("2026-04-08", &[(1001, 100.0)], &[]).await;
    api.eat("2026-04-10", &[], &[(&vitamin_d, 1.0)]).await;

    let week = api.summary("2026-04-06", "2026-04-12", "week").await;
    let period = &week["periods"][0];
    assert_eq!(
        json!([
            period["logged_days"],
            period["missing_days"],
            period["intake_days"]
        ]),
        json!([3, 4, 2])
    );
    assert_eq!(period["energy"]["eaten_kcal"], 71.0);
    let day = api.summary("2026-04-10", "2026-04-10", "day").await;
    assert_eq!(day["periods"][0]["energy"]["eaten_kcal"], Value::Null);
    assert_eq!(day["periods"][0]["protein"]["eaten_g"], 12.0);
    assert_eq!(day["periods"][0]["macro_energy"]["protein_kcal"], 48.0);
    let coverage = &period["micronutrients"];
    assert_eq!(coverage["logged_days"], 3);
    assert_eq!(
        period_coverage(coverage, "vitamin_c"),
        json!([78.9, 2, null, null])
    );
    assert_eq!(
        period_coverage(coverage, "folate"),
        json!([13.6, 2, 0, false])
    );
    assert_eq!(
        period_coverage(coverage, "vitamin_k"),
        json!([null, 0, null, null])
    );
    assert_eq!(
        period_coverage(coverage, "vitamin_d"),
        json!([200.0, 1, 0, false])
    );

    let days = api.summary("2026-04-06", "2026-04-08", "day").await;
    let percents: Vec<Value> = days["periods"]
        .as_array()
        .unwrap()
        .iter()
        .map(|period| {
            json!([
                period["micronutrients"]["logged_days"],
                nutrient(&period["micronutrients"], "vitamin_c")["percent_of_recommended"],
            ])
        })
        .collect();
    assert_eq!(
        percents,
        vec![json!([1, 105.3]), json!([0, null]), json!([1, 52.6])]
    );

    let range = &week["micronutrients"];
    assert_eq!(range["state"], "available");
    assert_eq!(range["logged_days"], 3);
    let vitamin_c = nutrient(range, "vitamin_c");
    assert_eq!(
        json!([
            vitamin_c["name"],
            vitamin_c["group"],
            vitamin_c["unit"],
            vitamin_c["adequate_intake"],
            vitamin_c["percent_of_recommended"],
            vitamin_c["averaged_days"],
        ]),
        json!(["Vitamin C", "vitamins", "mg", false, 78.9, 2])
    );
    assert_eq!(nutrient(range, "epa_dha")["unit"], "mg");
}

#[tokio::test]
async fn a_lasting_shortfall_is_a_range_mean_below_70_percent_over_at_least_7_logged_days() {
    let api = Api::start().await;
    woman(&api).await;
    let days = [
        "2026-04-01",
        "2026-04-02",
        "2026-04-03",
        "2026-04-05",
        "2026-04-06",
        "2026-04-08",
    ];
    for day in days {
        api.eat(day, &[(1001, 100.0)], &[]).await;
    }

    let range = api.summary("2026-04-01", "2026-04-10", "week").await["micronutrients"].clone();
    assert_eq!(
        range["shortfall"],
        json!({"below_percent": 70.0, "min_logged_days": 7})
    );
    assert_eq!(nutrient(&range, "vitamin_c")["lasting_shortfall"], false);

    api.eat("2026-04-09", &[(1001, 100.0)], &[]).await;
    let range = api.summary("2026-04-01", "2026-04-10", "week").await["micronutrients"].clone();
    let shortfall = |key| {
        let nutrient = nutrient(&range, key);
        json!([
            nutrient["percent_of_recommended"],
            nutrient["lasting_shortfall"]
        ])
    };
    assert_eq!(shortfall("vitamin_c"), json!([52.6, true]));
    assert_eq!(shortfall("folate"), json!([9.1, true]));
    assert_eq!(shortfall("vitamin_d"), json!([null, false]));

    api.eat("2026-04-10", &[(1001, 900.0)], &[]).await;
    let range = api.summary("2026-04-01", "2026-04-10", "week").await["micronutrients"].clone();
    let vitamin_c = nutrient(&range, "vitamin_c");
    assert_eq!(
        json!([
            vitamin_c["percent_of_recommended"],
            vitamin_c["lasting_shortfall"]
        ]),
        json!([105.3, false])
    );
}

#[tokio::test]
async fn summary_counts_days_above_upper_limits_and_cut_down_windows_as_the_day_view_does() {
    let api = Api::start().await;
    woman(&api).await;
    let strong = api
        .supplement(
            "Vitamin D 150",
            json!({"amounts": [{"key": "vitamin_d", "value": 150, "unit": "µg"}]}),
        )
        .await;
    let magnesium = api
        .supplement(
            "Magnesium",
            json!({"amounts": [{"key": "magnesium", "value": 300, "unit": "mg"}]}),
        )
        .await;
    api.eat("2026-04-06", &[(1002, 10.0)], &[]).await;
    api.eat("2026-04-07", &[], &[(&strong, 1.0)]).await;
    api.eat("2026-04-08", &[(1003, 100.0)], &[]).await;
    api.eat("2026-04-09", &[], &[(&strong, 1.0)]).await;
    api.eat("2026-04-15", &[(1003, 100.0)], &[(&magnesium, 1.0)])
        .await;
    assert_eq!(
        api.nutrient("2026-04-07", "vitamin_d").await["upper_limit_status"],
        "caution"
    );
    assert_eq!(
        api.nutrient("2026-04-09", "vitamin_d").await["upper_limit_status"],
        "cut_down"
    );

    let summary = api.summary("2026-04-06", "2026-04-26", "week").await;
    let weeks: Vec<Value> = summary["periods"]
        .as_array()
        .unwrap()
        .iter()
        .map(|period| {
            let coverage = &period["micronutrients"];
            json!([
                period["start"],
                limit_flags(coverage, "vitamin_d"),
                limit_flags(coverage, "magnesium"),
            ])
        })
        .collect();
    assert_eq!(
        weeks,
        vec![
            json!(["2026-04-06", [2, true], [0, false]]),
            json!(["2026-04-13", [0, false], [1, false]]),
            json!(["2026-04-20", [0, false], [0, false]]),
        ]
    );

    let range = &summary["micronutrients"];
    assert_eq!(limit_flags(range, "vitamin_d"), json!([2, true]));
    assert_eq!(limit_flags(range, "magnesium"), json!([1, false]));
    assert_eq!(limit_flags(range, "vitamin_c"), json!([null, null]));
}

#[tokio::test]
async fn summary_coverage_is_unavailable_with_the_reason_while_the_rest_still_works() {
    let api = Api::start().await;
    api.eat("2026-04-08", &[(1001, 200.0)], &[]).await;

    let summary = api.summary("2026-04-06", "2026-04-12", "week").await;
    assert_eq!(
        summary["micronutrients"],
        json!({
            "state": "unavailable",
            "missing": ["sex", "birth_date"],
            "reason": "The micronutrient panel needs the Profile's sex (male or female) and birth date",
        })
    );
    let period = &summary["periods"][0];
    assert_eq!(period["micronutrients"], Value::Null);
    assert_eq!(period["energy"]["eaten_kcal"], 94.0);

    api.profile(json!({"sex": "female", "birth_date": "2010-01-01"}))
        .await;
    let summary = api.summary("2026-04-06", "2026-04-12", "week").await;
    assert_eq!(summary["micronutrients"]["state"], "unavailable");
    assert_eq!(summary["micronutrients"]["missing"], json!([]));
    assert_eq!(
        summary["micronutrients"]["reason"],
        "The micronutrient panel covers adults from 18; on 2026-04-12 the Profile's birth date \
         gives 16"
    );
    assert_eq!(summary["periods"][0]["micronutrients"], Value::Null);
}
