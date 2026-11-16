//! The Nutrition Summary targets and range rules, from a real `app.db` and a real `food.db`
//! imported from a synthetic DTU-shaped workbook served by `wiremock`.

mod common;
#[path = "nutrition_support/summary.rs"]
mod support;

use axum::{http::Method, http::StatusCode};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use support::{Api, CHICKEN, day_from_today};

#[tokio::test]
async fn targets_are_the_mean_of_each_days_target_and_reading_fixes_past_weeks() {
    let api = Api::start().await;
    // A 30-year-old man of 180 cm at 80 kg: BMR 1,780 kcal and a 2,670 kcal prior.
    api.fitness(
        Method::PATCH,
        "/api/profile",
        json!({"height_cm": 180, "sex": "male", "birth_date": "1996-01-15"}),
    )
    .await;
    api.weigh("2026-03-01", 80.0).await;
    // 2,000 kcal a day in the week of 6 April: 1,333.3 g of chicken with 280 g protein.
    for day in 6..=12 {
        api.eat(&format!("2026-04-{day:02}"), &[(CHICKEN, 2000.0 / 1.5)])
            .await;
    }

    let summary = api.summary("2026-04-06", "2026-04-19", "week").await;
    let periods = summary["periods"].as_array().unwrap();
    let series: Vec<Value> = periods
        .iter()
        .map(|period| json!({"energy": period["energy"], "protein": period["protein"]}))
        .collect();
    assert_eq!(
        series,
        vec![
            // The prior less 550 kcal a day for losing 0.5 kg a week.
            json!({
                "energy": {
                    "eaten_kcal": 2000.0,
                    "calorie_target_kcal": 2120.0,
                    "estimated_expenditure_kcal": 2670.0,
                },
                "protein": {"eaten_g": 280.0, "target_g": 176.0},
            }),
            // Seven logged days blend a 2,000 kcal data estimate half and half with the prior.
            json!({
                "energy": {
                    "eaten_kcal": null,
                    "calorie_target_kcal": 1785.0,
                    "estimated_expenditure_kcal": 2335.0,
                },
                "protein": {"eaten_g": null, "target_g": 176.0},
            }),
        ]
    );

    // The summary fixed both weeks' records, so a later backfill into the first week's window
    // changes neither the Day view nor a second summary.
    api.eat("2026-03-20", &[(CHICKEN, 3000.0)]).await;
    let (status, day) = api
        .send(Method::GET, "/api/nutrition/days/2026-04-08", None)
        .await;
    assert_eq!(status, StatusCode::OK, "{day}");
    assert_eq!(day["targets"]["calorie_target_kcal"], 2120.0);
    let again = api.summary("2026-04-06", "2026-04-19", "week").await;
    assert_eq!(again["periods"][0]["energy"]["calorie_target_kcal"], 2120.0);
    assert_eq!(again["periods"][1]["energy"]["calorie_target_kcal"], 1785.0);
}

#[tokio::test]
async fn without_the_profile_targets_are_null_while_intake_and_weight_remain() {
    let api = Api::start().await;
    api.weigh("2026-04-02", 80.0).await;
    api.eat("2026-04-02", &[(CHICKEN, 1000.0)]).await;

    let summary = api.summary("2026-04-02", "2026-04-02", "day").await;
    let period = &summary["periods"][0];

    assert_eq!(
        period["energy"],
        json!({
            "eaten_kcal": 1500.0,
            "calorie_target_kcal": null,
            "estimated_expenditure_kcal": null,
        })
    );
    assert_eq!(
        period["protein"],
        json!({"eaten_g": 210.0, "target_g": null})
    );
    assert_eq!(period["weight"]["trend_weight_kg"], 80.0);
}

#[tokio::test]
async fn the_range_is_one_to_365_days_ending_by_today_with_a_known_aggregation() {
    let api = Api::start().await;
    let today = day_from_today(0);
    let first_of_year = day_from_today(-364);

    let year = api.summary(&first_of_year, &today, "month").await;
    assert_eq!(year["from"], first_of_year);
    let days: i64 = year["periods"]
        .as_array()
        .unwrap()
        .iter()
        .map(|period| period["days"].as_i64().unwrap())
        .sum();
    assert_eq!(days, 365);
    assert_eq!(year["missing_days"], 365);

    let too_long = api
        .try_summary(&day_from_today(-365), &today, "month")
        .await;
    let backwards = api.try_summary("2026-04-02", "2026-04-01", "day").await;
    let future = api.try_summary(&today, &day_from_today(1), "day").await;
    let unknown = api.try_summary("2026-04-01", "2026-04-02", "year").await;
    let bad_day = api.try_summary("2026-4-1", "2026-04-02", "day").await;
    let missing = api
        .send(Method::GET, "/api/nutrition/summary?from=2026-04-01", None)
        .await;

    for (status, body) in [&too_long, &backwards, &future, &unknown, &bad_day, &missing] {
        assert_eq!(*status, StatusCode::BAD_REQUEST, "{body}");
    }
    assert_eq!(
        too_long.1["error"],
        "from and to must cover 1 to 365 calendar days, with from <= to"
    );
    assert_eq!(
        future.1["error"],
        format!("to must not be after today ({today}) in Europe/Copenhagen")
    );
}
