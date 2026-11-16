//! The Nutrition Summary over a range, by day, ISO week or calendar month: intake means over
//! logged days, the Calorie Target and Estimated Expenditure, protein, energy by macronutrient,
//! the NOVA 1 share, Trend Weight and scale weight, and the weekly weight change against the
//! Nutrition Goal. A real `app.db` is seeded through the Profile, Body Weight, NOVA Class and Meal
//! Log routes, and a real `food.db` is imported from a synthetic DTU-shaped workbook served by
//! `wiremock`.
//!
//! Synthetic Chicken breast (Food 907) has 150 kcal, 21 g protein and 7 g fat per 100 g and no
//! carbohydrate value; Banana (Food 3) has 90 kcal and 20 g carbohydrate per 100 g.

mod common;
#[path = "nutrition_support/summary.rs"]
mod support;

use axum::{http::Method, http::StatusCode};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use support::{Api, day_from_today, three_logged_days};

#[tokio::test]
async fn weeks_run_monday_to_sunday_and_edge_weeks_cover_only_the_range() {
    let api = Api::start().await;
    three_logged_days(&api).await;

    // Wednesday 1 April to Tuesday 14 April.
    let summary = api.summary("2026-04-01", "2026-04-14", "week").await;

    assert_eq!(summary["from"], "2026-04-01");
    assert_eq!(summary["to"], "2026-04-14");
    assert_eq!(summary["aggregation"], "week");
    assert_eq!(summary["time_zone"], "Europe/Copenhagen");
    assert_eq!(summary["logged_days"], 3);
    assert_eq!(summary["missing_days"], 11);
    // The values' credit: the Food database version the Portions copied, and no Product.
    assert_eq!(
        summary["sources"],
        json!([{
            "name": "The Danish Food Composition Database",
            "version": "1",
            "credit": "© The Danish Food Composition Database, version 1",
            "licence": "CC BY 4.0",
        }])
    );
    assert_eq!(summary["label_source"], Value::Null);
    let periods = summary["periods"].as_array().unwrap();
    let bounds: Vec<Value> = periods
        .iter()
        .map(|period| {
            json!([
                period["start"],
                period["end"],
                period["days"],
                period["partial"],
                period["logged_days"],
                period["missing_days"],
            ])
        })
        .collect();
    assert_eq!(
        bounds,
        vec![
            json!(["2026-04-01", "2026-04-05", 5, true, 2, 3]),
            json!(["2026-04-06", "2026-04-12", 7, false, 1, 6]),
            json!(["2026-04-13", "2026-04-14", 2, true, 0, 2]),
        ]
    );

    // The first week's means cover its two logged days, never its three missing ones.
    let first = &periods[0];
    assert_eq!(first["energy"]["eaten_kcal"], 1425.0);
    assert_eq!(first["protein"]["eaten_g"], 168.0);
    assert_eq!(
        first["macro_energy"],
        json!({
            "protein_kcal": 672.0,
            "carbohydrate_kcal": 400.0,
            "fat_kcal": 504.0,
            "alcohol_kcal": null,
        })
    );
    // Only Saturday knows carbohydrate (100 g); Thursday is unknown rather than zero.
    assert_eq!(
        first["known_days"],
        json!({
            "energy": 2, "protein": 2, "carbohydrate": 1, "fat": 2, "alcohol": 0,
        })
    );
    // NOVA 1 energy is summed over the period before taking the share: 2,400 of 2,850 kcal.
    assert_eq!(
        first["nova"],
        json!({
            "nova_1_share": 0.8421,
            "nova_1_kcal": 2400.0,
            "classified_kcal": 2850.0,
            "unclassified_portions": 0,
        })
    );
    assert_eq!(periods[1]["energy"]["eaten_kcal"], 1500.0);
    assert_eq!(periods[1]["nova"]["nova_1_share"], 1.0);

    // A week without Meal Logs is missing: no intake, not zero.
    let last = &periods[2];
    assert_eq!(last["energy"]["eaten_kcal"], Value::Null);
    assert_eq!(last["protein"]["eaten_g"], Value::Null);
    assert_eq!(
        last["macro_energy"],
        json!({
            "protein_kcal": null,
            "carbohydrate_kcal": null,
            "fat_kcal": null,
            "alcohol_kcal": null,
        })
    );
    assert_eq!(last["nova"]["nova_1_share"], Value::Null);
}

#[tokio::test]
async fn days_without_meal_logs_are_missing_and_months_are_calendar_months() {
    let api = Api::start().await;
    three_logged_days(&api).await;

    let days = api.summary("2026-04-01", "2026-04-03", "day").await;
    let energy: Vec<Value> = days["periods"]
        .as_array()
        .unwrap()
        .iter()
        .map(|period| {
            json!([
                period["start"],
                period["end"],
                period["partial"],
                period["logged_days"],
                period["missing_days"],
                period["energy"]["eaten_kcal"],
            ])
        })
        .collect();
    assert_eq!(
        energy,
        vec![
            json!(["2026-04-01", "2026-04-01", false, 0, 1, null]),
            json!(["2026-04-02", "2026-04-02", false, 1, 0, 1500.0]),
            json!(["2026-04-03", "2026-04-03", false, 0, 1, null]),
        ]
    );

    // Monday 30 March to Saturday 2 May: the edge months cover only their days in the range.
    let months = api.summary("2026-03-30", "2026-05-02", "month").await;
    let periods: Vec<Value> = months["periods"]
        .as_array()
        .unwrap()
        .iter()
        .map(|period| {
            json!([
                period["start"],
                period["end"],
                period["days"],
                period["partial"],
                period["logged_days"],
                period["energy"]["eaten_kcal"],
            ])
        })
        .collect();
    assert_eq!(
        periods,
        vec![
            json!(["2026-03-30", "2026-03-31", 2, true, 0, null]),
            json!(["2026-04-01", "2026-04-30", 30, false, 3, 1450.0]),
            json!(["2026-05-01", "2026-05-02", 2, true, 0, null]),
        ]
    );
}

#[tokio::test]
async fn trend_weight_scale_weight_and_the_weekly_change_follow_the_weigh_ins() {
    let api = Api::start().await;
    // Trend Weight: 80 from 29 March, 80.2 from 5 April and 80.38 from 12 April.
    api.weigh("2026-03-29", 80.0).await;
    api.weigh("2026-04-05", 82.0).await;
    api.weigh("2026-04-12", 82.0).await;

    assert_eq!(
        api.series("2026-03-30", "2026-04-12", "week", "weight")
            .await,
        vec![
            json!({"start": "2026-03-30", "end": "2026-04-05", "weight": {
                "trend_weight_kg": 80.2, "scale_weight_kg": 82.0, "weigh_ins": 1,
            }}),
            json!({"start": "2026-04-06", "end": "2026-04-12", "weight": {
                "trend_weight_kg": 80.38, "scale_weight_kg": 82.0, "weigh_ins": 1,
            }}),
        ]
    );
    // A week's change runs from the Sunday before to its own Sunday; the goal is the default
    // loss of 0.5 kg per week.
    assert_eq!(
        api.series("2026-03-30", "2026-04-12", "week", "weight_change")
            .await,
        vec![
            json!({"start": "2026-03-30", "end": "2026-04-05", "weight_change": {
                "kg_per_week": 0.2, "goal_kg_per_week": -0.5,
            }}),
            json!({"start": "2026-04-06", "end": "2026-04-12", "weight_change": {
                "kg_per_week": 0.18, "goal_kg_per_week": -0.5,
            }}),
        ]
    );

    // A day's change covers the 7 days ending on it, and needs a Trend Weight a week before.
    let days = api.summary("2026-04-04", "2026-04-06", "day").await;
    let changes: Vec<Value> = days["periods"]
        .as_array()
        .unwrap()
        .iter()
        .map(|period| {
            json!([
                period["start"],
                period["weight"]["trend_weight_kg"],
                period["weight"]["scale_weight_kg"],
                period["weight_change"]["kg_per_week"],
            ])
        })
        .collect();
    assert_eq!(
        changes,
        vec![
            json!(["2026-04-04", 80.0, null, null]),
            json!(["2026-04-05", 80.2, 82.0, 0.2]),
            json!(["2026-04-06", 80.2, null, 0.2]),
        ]
    );

    // A month's change is its whole change per week: 0.38 kg over April's 30 days. March's 3 days
    // in the range are shorter than a week, so they take the week ending on their last day,
    // which starts before the first weigh-in.
    let months = api.summary("2026-03-29", "2026-04-30", "month").await;
    let changes: Vec<Value> = months["periods"]
        .as_array()
        .unwrap()
        .iter()
        .map(|period| {
            json!([
                period["start"],
                period["weight"]["scale_weight_kg"],
                period["weight"]["weigh_ins"],
                period["weight_change"]["kg_per_week"],
            ])
        })
        .collect();
    assert_eq!(
        changes,
        vec![
            json!(["2026-03-29", 80.0, 1, null]),
            json!(["2026-04-01", 82.0, 2, 0.09]),
        ]
    );
}

#[tokio::test]
async fn the_goal_rate_follows_the_goal_in_effect_each_day() {
    let api = Api::start().await;
    let (status, body) = api
        .send(
            Method::PUT,
            "/api/nutrition/goal",
            Some(json!({"direction": "lean_bulk", "rate_kg_per_week": 0.25})),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (yesterday, today) = (day_from_today(-1), day_from_today(0));

    assert_eq!(
        api.series(&yesterday, &today, "day", "weight_change").await,
        vec![
            json!({"start": yesterday, "end": yesterday, "weight_change": {
                "kg_per_week": null, "goal_kg_per_week": -0.5,
            }}),
            json!({"start": today, "end": today, "weight_change": {
                "kg_per_week": null, "goal_kg_per_week": 0.25,
            }}),
        ]
    );
}
