//! The micronutrient panel through the Nutrition Day, and micronutrient coverage through the
//! Nutrition Summary: NNR 2023 Recommended Intakes by the Profile's sex and age band, and EFSA
//! Upper Limits checked against the day and against the trailing 7-day average, from a real `app.db` seeded through the Profile, Supplement and Meal
//! Log routes, and a real `food.db` imported from a synthetic DTU-shaped workbook served by
//! `wiremock`. Every Food and value here is invented.

mod common;
#[path = "nutrition_support/micronutrients.rs"]
mod support;

use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use support::{Api, coverage, limits, nutrient, reference, woman};

#[tokio::test]
async fn the_panel_is_unavailable_without_the_profile_sex_and_birth_date_or_below_adult_age() {
    let api = Api::start().await;

    let panel = api.panel("2026-04-08").await;
    assert_eq!(panel["state"], "unavailable");
    assert_eq!(panel["missing"], json!(["sex", "birth_date"]));
    assert_eq!(
        panel["reason"],
        "The micronutrient panel needs the Profile's sex (male or female) and birth date"
    );

    api.profile(json!({"sex": "Female"})).await;
    let panel = api.panel("2026-04-08").await;
    assert_eq!(panel["missing"], json!(["birth_date"]));
    assert_eq!(
        panel["reason"],
        "The micronutrient panel needs the Profile's birth date"
    );

    // 17 on the day read: NNR 2023's adult references start at 18.
    api.profile(json!({"birth_date": "2008-06-01"})).await;
    let panel = api.panel("2026-04-08").await;
    assert_eq!(panel["state"], "unavailable");
    assert_eq!(panel["missing"], json!([]));
    assert_eq!(
        panel["reason"],
        "The micronutrient panel covers adults from 18; on 2026-04-08 the Profile's birth date \
         gives 17"
    );

    // The age follows the day read, so the panel starts on the 18th birthday.
    let panel = api.panel("2026-06-01").await;
    assert_eq!(panel["state"], "available");
    assert_eq!(panel["sex"], "female");
    assert_eq!(panel["age"], 18);
    assert_eq!(panel["age_band"], "18-24");
}

#[tokio::test]
async fn sex_and_the_age_band_on_the_day_choose_the_recommended_intakes() {
    let api = Api::start().await;
    api.profile(json!({"sex": "female", "birth_date": "1996-01-15"}))
        .await;

    // A woman of 30: NNR 2023 25-50 years.
    let panel = api.panel("2026-04-08").await;
    assert_eq!(
        (
            panel["state"].clone(),
            panel["age"].clone(),
            panel["age_band"].clone()
        ),
        (json!("available"), json!(30), json!("25-50"))
    );
    assert_eq!(panel["reference_energy_mj"], 9.0);
    assert_eq!(
        reference(&nutrient(&panel, "vitamin_c")),
        (json!(95.0), json!(false))
    );
    assert_eq!(
        reference(&nutrient(&panel, "vitamin_d")),
        (json!(10.0), json!(false))
    );
    assert_eq!(
        reference(&nutrient(&panel, "iron")),
        (json!(15.0), json!(false))
    );
    assert_eq!(
        reference(&nutrient(&panel, "magnesium")),
        (json!(300.0), json!(true))
    );
    // Thiamin and niacin are set per MJ: 0.1 mg and 1.6 mg NE × 9.0 MJ.
    assert_eq!(
        reference(&nutrient(&panel, "vitamin_b1")),
        (json!(0.9), json!(false))
    );
    assert_eq!(
        reference(&nutrient(&panel, "niacin_equivalent")),
        (json!(14.4), json!(false))
    );
    assert_eq!(nutrient(&panel, "vitamin_b1")["unit"], "mg");
    assert_eq!(nutrient(&panel, "niacin_equivalent")["unit"], "mg NE");

    // A man of 30.
    api.profile(json!({"sex": "male"})).await;
    let panel = api.panel("2026-04-08").await;
    assert_eq!(panel["sex"], "male");
    assert_eq!(panel["reference_energy_mj"], 11.3);
    assert_eq!(
        reference(&nutrient(&panel, "vitamin_c")),
        (json!(110.0), json!(false))
    );
    assert_eq!(
        reference(&nutrient(&panel, "iron")),
        (json!(9.0), json!(false))
    );
    assert_eq!(
        reference(&nutrient(&panel, "magnesium")),
        (json!(350.0), json!(true))
    );
    assert_eq!(
        reference(&nutrient(&panel, "vitamin_b1")),
        (json!(1.13), json!(false))
    );

    // A man who turns 71 on 2026-04-10 moves from 51-70 to 71+ years that day.
    api.profile(json!({"birth_date": "1955-04-10"})).await;
    let panel = api.panel("2026-04-09").await;
    assert_eq!(
        (panel["age"].clone(), panel["age_band"].clone()),
        (json!(70), json!("51-70"))
    );
    assert_eq!(
        reference(&nutrient(&panel, "vitamin_d")),
        (json!(10.0), json!(false))
    );
    let panel = api.panel("2026-04-10").await;
    assert_eq!(
        (panel["age"].clone(), panel["age_band"].clone()),
        (json!(71), json!("71+"))
    );
    assert_eq!(panel["reference_energy_mj"], 10.1);
    assert_eq!(
        reference(&nutrient(&panel, "vitamin_d")),
        (json!(20.0), json!(false))
    );
    assert_eq!(
        reference(&nutrient(&panel, "vitamin_a")),
        (json!(750.0), json!(false))
    );
    assert_eq!(
        reference(&nutrient(&panel, "vitamin_b1")),
        (json!(1.01), json!(false))
    );
}

#[tokio::test]
async fn each_nutrient_is_a_percentage_of_its_recommended_intake_split_into_food_and_supplements() {
    let api = Api::start().await;
    woman(&api).await;
    let vitamin_d = api
        .supplement(
            "Vitamin D3",
            json!({"amounts": [{"key": "vitamin_d", "value": 10, "unit": "µg"}]}),
        )
        .await;
    // 200 g of Orange and 100 g of Salmon with one 10 µg vitamin D capsule.
    api.eat(
        "2026-04-08",
        &[(1001, 200.0), (1002, 100.0)],
        &[(&vitamin_d, 1.0)],
    )
    .await;

    let panel = api.panel("2026-04-08").await;
    assert_eq!(
        coverage(&nutrient(&panel, "vitamin_c")),
        json!([100.0, 100.0, null, 105.3])
    );
    assert_eq!(
        coverage(&nutrient(&panel, "vitamin_d")),
        json!([20.0, 10.0, 10.0, 200.0])
    );
    assert_eq!(
        coverage(&nutrient(&panel, "folate")),
        json!([60.0, 60.0, null, 18.2])
    );
    // Neither Food has a vitamin K value: unknown, not zero.
    let vitamin_k = nutrient(&panel, "vitamin_k");
    assert_eq!(coverage(&vitamin_k), json!([null, null, null, null]));
    assert_eq!(vitamin_k["unknown_portions"], 2);
    assert_eq!(vitamin_k["adequate_intake"], true);
    assert_eq!(vitamin_k["source"], "NNR 2023 Table 13");

    // A day without Meal Logs knows no amounts.
    let empty = api.panel("2026-04-09").await;
    assert_eq!(empty["state"], "available");
    assert_eq!(
        coverage(&nutrient(&empty, "vitamin_c")),
        json!([null, null, null, null])
    );
}

#[tokio::test]
async fn preformed_vitamin_a_counts_retinol_from_food_and_supplements_but_not_beta_carotene() {
    let api = Api::start().await;
    woman(&api).await;
    let beta_carotene = api
        .supplement(
            "Beta-carotene",
            json!({"amounts": [
                {"key": "vitamin_a", "form": "beta_carotene", "value": 2000, "unit": "µg"},
            ]}),
        )
        .await;
    let retinol = api
        .supplement(
            "Retinol",
            json!({"amounts": [
                {"key": "vitamin_a", "form": "retinol", "value": 1500, "unit": "µg"},
            ]}),
        )
        .await;
    // 50 g of Liver pate is 2,000 µg RE, all of it retinol.
    api.eat("2026-05-20", &[(1004, 50.0)], &[(&beta_carotene, 1.0)])
        .await;
    api.eat("2026-05-21", &[(1004, 50.0)], &[(&retinol, 1.0)])
        .await;

    let vitamin_a = api.nutrient("2026-05-20", "vitamin_a").await;
    assert_eq!(coverage(&vitamin_a), json!([4000.0, 2000.0, 2000.0, 571.4]));
    assert_eq!(
        limits(&vitamin_a),
        json!([["preformed_vitamin_a", 3000.0, 2000.0, 2000.0, 1, "none"]])
    );
    let vitamin_a = api.nutrient("2026-05-21", "vitamin_a").await;
    assert_eq!(
        limits(&vitamin_a),
        json!([["preformed_vitamin_a", 3000.0, 3500.0, 2750.0, 2, "caution"]])
    );
}

#[tokio::test]
async fn epa_and_dha_from_food_and_supplements_add_up_against_the_efsa_adequate_intake() {
    let api = Api::start().await;
    woman(&api).await;
    let fish_oil = api
        .supplement(
            "Fish oil",
            json!({
                "amounts": [
                    {"key": "epa", "value": 180, "unit": "mg"},
                    {"key": "dha", "value": 120, "unit": "mg"},
                ],
                "compounds": [{"name": "Creatine monohydrate", "value": 3, "unit": "g"}],
            }),
        )
        .await;
    // 10 g of Salmon: 50 mg EPA and 120 mg DHA; one capsule: 180 mg EPA and 120 mg DHA.
    api.eat("2026-04-08", &[(1002, 10.0)], &[(&fish_oil, 1.0)])
        .await;

    let day = api.day("2026-04-08").await;
    let panel = &day["micronutrients"];
    let omega_3 = nutrient(panel, "epa_dha");
    assert_eq!(
        json!([
            omega_3["name"],
            omega_3["unit"],
            omega_3["recommended_intake"],
            omega_3["adequate_intake"],
            omega_3["upper_limits"],
            omega_3["upper_limit_status"],
        ]),
        json!(["EPA+DHA", "mg", 250.0, true, [], null])
    );
    assert_eq!(coverage(&omega_3), json!([470.0, 170.0, 300.0, 188.0]));

    // A compound without a reference is an amount taken, outside the panel.
    assert_eq!(
        day["compounds"],
        json!([{"name": "Creatine monohydrate", "value": 3.0, "unit": "g"}])
    );
    let keys: Vec<&Value> = panel["nutrients"]
        .as_array()
        .unwrap()
        .iter()
        .map(|nutrient| &nutrient["key"])
        .collect();
    assert!(!keys.contains(&&json!("creatine_monohydrate")), "{keys:?}");
}
