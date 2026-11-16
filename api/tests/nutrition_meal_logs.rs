//! Meal Logs and the Nutrition Day through the nutrition routes, over a real `app.db` and a real
//! `food.db` imported from a synthetic DTU-shaped workbook served by `wiremock`.

mod common;
#[path = "nutrition_support/meal_logs.rs"]
mod support;

use axum::http::StatusCode;
use common::food_workbook::second_dataset;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use support::{Nutrition, at, breakfast, nutrient};

#[tokio::test]
async fn a_logged_meal_keeps_scaled_snapshots_with_sources_and_totals() {
    let api = Nutrition::start().await;

    let meal_log = api.logged(breakfast("breakfast-1")).await;

    assert_eq!(meal_log["eaten_at"], "2026-10-29T08:15:00+01:00");
    assert_eq!(meal_log["day"], "2026-10-29");
    assert_eq!(meal_log["note"], "after the run");
    let skyr = &meal_log["portions"][0];
    assert_eq!(
        (
            &skyr["food_id"],
            &skyr["name_en"],
            &skyr["grams"],
            &skyr["dataset_version"]
        ),
        (
            &json!(1693),
            &json!("Skyr, 0.2 % fat"),
            &json!(150.0),
            &json!("1")
        )
    );
    assert_eq!(
        skyr["nutrients"],
        json!({
            "energy_kcal": {"value": 93.0, "unit": "kcal", "source": "food"},
            "protein": {"value": 16.5, "unit": "g", "source": "food"},
            "available_carbohydrate": {"value": 6.0, "unit": "g", "source": "food"},
            "fat": {"value": 0.3, "unit": "g", "source": "food"},
            "dietary_fibre": {"value": 0.0, "unit": "g", "source": "food"},
            "alcohol": {"value": 0.0, "unit": "g", "source": "food"},
        })
    );
    // The chicken has no carbohydrate, fibre or alcohol value: unknown, never zero.
    assert_eq!(
        meal_log["portions"][1]["nutrients"]["available_carbohydrate"],
        json!({"value": null, "unit": "g", "source": null})
    );
    assert_eq!(
        meal_log["totals"],
        json!({
            "energy_kcal": {"value": 273.0, "unit": "kcal", "unknown_portions": 0, "food": 273.0, "supplements": null},
            "protein": {"value": 41.7, "unit": "g", "unknown_portions": 0, "food": 41.7, "supplements": null},
            "available_carbohydrate": {"value": 6.0, "unit": "g", "unknown_portions": 1, "food": 6.0, "supplements": null},
            "fat": {"value": 8.7, "unit": "g", "unknown_portions": 0, "food": 8.7, "supplements": null},
            "dietary_fibre": {"value": 0.0, "unit": "g", "unknown_portions": 1, "food": 0.0, "supplements": null},
            "alcohol": {"value": 0.0, "unit": "g", "unknown_portions": 1, "food": 0.0, "supplements": null},
        })
    );
    assert_eq!(api.day("2026-10-29").await, [meal_log]);
}

#[tokio::test]
async fn nutrition_day_totals_every_parameter_with_units_and_credits_the_dataset() {
    let api = Nutrition::start().await;
    api.logged(breakfast("breakfast-1")).await;
    let lunch = api
        .logged(json!({
            "eaten_at": "2026-10-29T12:30:00+01:00",
            "portions": [{"food_id": 3, "grams": 100}],
            "request_id": "lunch-1",
        }))
        .await;

    let (status, day) = api.get("/api/nutrition/days/2026-10-29").await;

    assert_eq!(status, StatusCode::OK, "{day}");
    assert_eq!(
        (
            &day["day"],
            &day["time_zone"],
            &day["day_start"],
            &day["day_end"]
        ),
        (
            &json!("2026-10-29"),
            &json!("Europe/Copenhagen"),
            &json!("2026-10-29T00:00:00+01:00"),
            &json!("2026-10-30T00:00:00+01:00")
        )
    );
    assert_eq!(
        day["totals"]["energy_kcal"],
        json!({"value": 363.0, "unit": "kcal", "unknown_portions": 0, "food": 363.0, "supplements": null})
    );
    assert_eq!(
        day["totals"]["available_carbohydrate"],
        json!({"value": 26.0, "unit": "g", "unknown_portions": 1, "food": 26.0, "supplements": null})
    );
    // Every per-100 g parameter, in database order; waste, density and pH describe the Food.
    let keys: Vec<&str> = day["nutrients"]
        .as_array()
        .unwrap()
        .iter()
        .map(|nutrient| nutrient["key"].as_str().unwrap())
        .collect();
    assert_eq!(
        keys,
        [
            "energy_kj",
            "energy_kcal",
            "protein",
            "available_carbohydrate",
            "dietary_fibre",
            "fat",
            "alcohol",
            "vitamin_d",
            "glycerol",
            "epa"
        ]
    );
    assert_eq!(
        nutrient(&day, "vitamin_d"),
        &json!({
            "key": "vitamin_d", "name_da": "D-vitamin", "name_en": "Vitamin D",
            "group": {"name_da": "Vitaminer", "name_en": "Vitamins"},
            "unit": "µg", "value": 0.0, "unknown_portions": 2, "food": 0.0, "supplements": null,
        })
    );
    assert_eq!(nutrient(&day, "epa")["value"], 0.012);
    assert_eq!(
        (
            &nutrient(&day, "glycerol")["value"],
            &nutrient(&day, "glycerol")["unknown_portions"]
        ),
        (&Value::Null, &json!(3))
    );
    assert_eq!(day["meal_logs"].as_array().unwrap().len(), 2);
    assert_eq!(day["meal_logs"][1], lunch);
    assert_eq!(
        day["source"]["credit"],
        "© The Danish Food Composition Database, version 1"
    );

    let (_, empty) = api.get("/api/nutrition/days/2026-10-28").await;
    assert_eq!(empty["meal_logs"], json!([]));
    assert_eq!(
        empty["totals"]["energy_kcal"],
        json!({"value": null, "unit": "kcal", "unknown_portions": 0, "food": null, "supplements": null})
    );
    let (status, _) = api.get("/api/nutrition/days/2026-10-32").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn a_reimported_food_database_never_changes_past_meal_logs() {
    let mut api = Nutrition::start().await;
    let breakfast = api.logged(breakfast("breakfast-1")).await;
    let banana = api
        .logged(json!({
            "eaten_at": "2026-10-29T15:00:00+01:00",
            "portions": [{"food_id": 3, "grams": 120}],
            "request_id": "banana-1",
        }))
        .await;
    let (_, day_before) = api.get("/api/nutrition/days/2026-10-29").await;

    // Version 2 changes Skyr's protein and drops the Banana.
    api.reimport("2", second_dataset()).await;

    assert_eq!(api.day("2026-10-29").await, [breakfast, banana]);
    let (_, day_after) = api.get("/api/nutrition/days/2026-10-29").await;
    assert_eq!(day_after["totals"], day_before["totals"]);
    assert_eq!(day_after["nutrients"], day_before["nutrients"]);
    let later = api
        .logged(json!({
            "eaten_at": "2026-10-29T20:00:00+01:00",
            "portions": [{"food_id": 1693, "grams": 150}],
            "request_id": "breakfast-2",
        }))
        .await;
    assert_eq!(later["portions"][0]["nutrients"]["protein"]["value"], 18.0);
    assert_eq!(later["portions"][0]["dataset_version"], "2");
}

#[tokio::test]
async fn meal_logs_belong_to_the_copenhagen_day_of_their_time_across_midnight_and_daylight_saving()
{
    let api = Nutrition::start().await;
    // (eaten_at, Copenhagen time it is shown in, Copenhagen day)
    let cases = [
        // Midnight in winter time (UTC+1).
        (
            "2026-10-29T23:59:59+01:00",
            "2026-10-29T23:59:59+01:00",
            "2026-10-29",
        ),
        (
            "2026-10-29T23:30:00Z",
            "2026-10-30T00:30:00+01:00",
            "2026-10-30",
        ),
        // Summer time ends on 2026-10-25: that day lasts 25 hours.
        (
            "2026-10-24T22:00:00Z",
            "2026-10-25T00:00:00+02:00",
            "2026-10-25",
        ),
        (
            "2026-10-25T00:30:00Z",
            "2026-10-25T02:30:00+02:00",
            "2026-10-25",
        ),
        (
            "2026-10-25T01:30:00Z",
            "2026-10-25T02:30:00+01:00",
            "2026-10-25",
        ),
        (
            "2026-10-25T22:59:59Z",
            "2026-10-25T23:59:59+01:00",
            "2026-10-25",
        ),
        // Summer time starts on 2026-03-29: that day lasts 23 hours.
        (
            "2026-03-28T23:00:00Z",
            "2026-03-29T00:00:00+01:00",
            "2026-03-29",
        ),
        (
            "2026-03-29T21:59:59Z",
            "2026-03-29T23:59:59+02:00",
            "2026-03-29",
        ),
        (
            "2026-03-29T22:00:00Z",
            "2026-03-30T00:00:00+02:00",
            "2026-03-30",
        ),
    ];
    for (index, (eaten_at, shown, day)) in cases.into_iter().enumerate() {
        let meal_log = api.logged(at(eaten_at, &format!("day-{index}"))).await;
        assert_eq!(
            (&meal_log["eaten_at"], &meal_log["day"]),
            (&json!(shown), &json!(day)),
            "{eaten_at}"
        );
    }

    let times = |logs: Vec<Value>| -> Vec<String> {
        logs.iter()
            .map(|log| log["eaten_at"].as_str().unwrap().to_owned())
            .collect()
    };
    assert_eq!(
        times(api.day("2026-10-25").await),
        [
            "2026-10-25T00:00:00+02:00",
            "2026-10-25T02:30:00+02:00",
            "2026-10-25T02:30:00+01:00",
            "2026-10-25T23:59:59+01:00"
        ]
    );
    let (_, day) = api.get("/api/nutrition/days/2026-03-29").await;
    assert_eq!(
        (&day["day_start"], &day["day_end"]),
        (
            &json!("2026-03-29T00:00:00+01:00"),
            &json!("2026-03-30T00:00:00+02:00")
        )
    );
    assert_eq!(day["meal_logs"].as_array().unwrap().len(), 2);

    let (status, range) = api
        .get("/api/nutrition/meal-logs?from=2026-10-25&to=2026-10-30")
        .await;
    assert_eq!(status, StatusCode::OK, "{range}");
    assert_eq!(
        (&range["from"], &range["to"], &range["time_zone"]),
        (
            &json!("2026-10-25"),
            &json!("2026-10-30"),
            &json!("Europe/Copenhagen")
        )
    );
    assert_eq!(range["meal_logs"].as_array().unwrap().len(), 6);
}

#[tokio::test]
async fn backfilled_times_are_accepted_and_future_or_offsetless_times_are_refused() {
    let api = Nutrition::start().await;
    let a_year_ago = chrono::Utc::now() - chrono::TimeDelta::days(365);
    let backfilled = api
        .logged(at(&a_year_ago.to_rfc3339(), "forgotten-dinner"))
        .await;
    let day = openhome_api::services::training::CalendarDay::of(a_year_ago).to_string();
    assert_eq!(backfilled["day"], day);
    assert_eq!(api.day(&day).await, [backfilled]);

    let tomorrow = chrono::Utc::now() + chrono::TimeDelta::days(1);
    for (eaten_at, request_id) in [
        (tomorrow.to_rfc3339(), "tomorrow"),
        ("2026-10-29T08:15:00".to_owned(), "no-offset"),
        ("yesterday morning".to_owned(), "words"),
    ] {
        let (status, body) = api.log(at(&eaten_at, request_id)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{eaten_at}: {body}");
        assert!(
            body["error"].as_str().unwrap().contains("eaten_at"),
            "{body}"
        );
    }
}
