use crate::support::{Api, man, today};
use axum::http::StatusCode;
use chrono::{Datelike, Days};
use pretty_assertions::assert_eq;
use serde_json::json;

#[tokio::test]
async fn targets_are_unavailable_until_the_profile_and_a_body_weight_allow_them() {
    let api = Api::start().await;

    let targets = api.targets("2026-04-08").await;
    assert_eq!(targets["state"], "unavailable");
    assert_eq!(
        targets["missing"],
        json!(["height", "sex", "birth_date", "body_weight"])
    );
    assert_eq!(
        targets["reason"],
        "Targets need the Profile's height, sex (male or female) and birth date, and a Body \
         Weight on or before 2026-04-08"
    );

    api.profile(json!({"height_cm": 180, "sex": "other"})).await;
    let targets = api.targets("2026-04-08").await;
    assert_eq!(
        targets["missing"],
        json!(["sex", "birth_date", "body_weight"])
    );

    api.profile(json!({"sex": "Male", "birth_date": "1996-01-15"}))
        .await;
    api.weigh("2026-04-09", 80.0).await;
    let targets = api.targets("2026-04-08").await;
    assert_eq!(targets["missing"], json!(["body_weight"]));
    assert_eq!(
        targets["reason"],
        "Targets need a Body Weight on or before 2026-04-08"
    );

    let targets = api.targets("2026-04-09").await;
    assert_eq!(targets["state"], "available", "{targets}");
    assert_eq!(targets["trend_weight_kg"], 80.0);
    assert_eq!(targets["calorie_target_kcal"], 2120.0);
}

#[tokio::test]
async fn without_logged_days_the_expenditure_is_the_bmr_prior_and_macros_follow_trend_weight() {
    let api = Api::start().await;
    man(&api).await;
    api.weigh("2026-03-01", 80.0).await;

    let targets = api.targets("2026-04-08").await;

    assert_eq!(
        targets,
        json!({
            "state": "available",
            "week_start": "2026-04-06",
            "week_fixed": true,
            "trend_weight_kg": 80.0,
            "bmr_kcal": 1780.0,
            "estimated_expenditure_kcal": 2670.0,
            "expenditure_basis": {
                "window_start": "2026-03-09",
                "window_end": "2026-04-05",
                "logged_days": 0,
                "data_estimate_kcal": null,
                "prior_kcal": 2670.0,
                "blend_weight": 0.0,
            },
            "goal": {"direction": "lose", "rate_kg_per_week": 0.5, "effective_from": null},
            "calorie_target_kcal": 2120.0,
            "bmr_floor_applied": false,
            "eaten_kcal": 0.0,
            "remaining_kcal": 2120.0,
            "macros": {
                "protein": {"target_g": 176.0, "eaten_g": 0.0, "status": "below"},
                "fat": {"floor_g": 56.0, "eaten_g": 0.0, "status": "below"},
                "carbs": {"target_g": 228.0, "eaten_g": 0.0},
            },
        })
    );

    api.eat("2026-04-08", 1500.0).await;
    let targets = api.targets("2026-04-08").await;
    assert_eq!(targets["calorie_target_kcal"], 2120.0);
    assert_eq!(targets["eaten_kcal"], 1500.0);
    assert_eq!(targets["remaining_kcal"], 620.0);
    assert_eq!(
        targets["macros"],
        json!({
            "protein": {"target_g": 176.0, "eaten_g": 210.0, "status": "met"},
            "fat": {"floor_g": 56.0, "eaten_g": 70.0, "status": "met"},
            "carbs": {"target_g": 228.0, "eaten_g": 0.0},
        })
    );
}

#[tokio::test]
async fn fewer_than_fourteen_logged_days_blend_the_data_estimate_with_the_prior() {
    let api = Api::start().await;
    man(&api).await;
    api.weigh("2026-03-01", 80.0).await;
    api.eat_daily("2026-03-23", "2026-03-29", 2000.0).await;
    api.eat("2026-03-08", 5000.0).await;
    api.eat("2026-04-06", 5000.0).await;

    let targets = api.targets("2026-04-08").await;

    assert_eq!(
        targets["expenditure_basis"],
        json!({
            "window_start": "2026-03-09",
            "window_end": "2026-04-05",
            "logged_days": 7,
            "data_estimate_kcal": 2000.0,
            "prior_kcal": 2670.0,
            "blend_weight": 0.5,
        })
    );
    assert_eq!(targets["estimated_expenditure_kcal"], 2335.0);
    assert_eq!(targets["calorie_target_kcal"], 1785.0);
}

#[tokio::test]
async fn fourteen_logged_days_make_the_expenditure_intake_corrected_by_the_trend_weight_change() {
    let api = Api::start().await;
    man(&api).await;
    api.weigh("2026-03-01", 80.0).await;
    api.weigh_daily("2026-03-09", "2026-04-05", 79.0).await;
    api.eat_daily("2026-03-12", "2026-03-31", 2500.0).await;

    let targets = api.targets("2026-04-08").await;

    assert_eq!(
        targets["expenditure_basis"],
        json!({
            "window_start": "2026-03-09",
            "window_end": "2026-04-05",
            "logged_days": 20,
            "data_estimate_kcal": 2761.0,
            "prior_kcal": 2656.0,
            "blend_weight": 1.0,
        })
    );
    assert_eq!(targets["trend_weight_kg"], 79.05);
    assert_eq!(targets["bmr_kcal"], 1771.0);
    assert_eq!(targets["estimated_expenditure_kcal"], 2761.0);
    assert_eq!(targets["calorie_target_kcal"], 2211.0);
    assert_eq!(targets["macros"]["protein"]["target_g"], 173.9);
    assert_eq!(targets["macros"]["fat"]["floor_g"], 55.3);
    assert_eq!(targets["macros"]["carbs"]["target_g"], 254.2);
}

#[tokio::test]
async fn a_week_keeps_its_target_when_meals_are_backfilled_after_its_first_read() {
    let api = Api::start().await;
    man(&api).await;
    api.weigh("2026-03-01", 80.0).await;
    let before = api.targets("2026-04-08").await;
    assert_eq!(before["calorie_target_kcal"], 2120.0);

    api.eat_daily("2026-03-10", "2026-03-23", 2000.0).await;

    assert_eq!(api.targets("2026-04-08").await, before);
    let sunday = api.targets("2026-04-12").await;
    assert_eq!(sunday["estimated_expenditure_kcal"], 2670.0);
    assert_eq!(sunday["calorie_target_kcal"], 2120.0);
    let next = api.targets("2026-04-13").await;
    assert_eq!(next["expenditure_basis"]["logged_days"], 8);
    assert_eq!(next["estimated_expenditure_kcal"], 2287.0);
    assert_eq!(next["calorie_target_kcal"], 1780.0);
}

#[tokio::test]
async fn the_calorie_target_never_falls_below_bmr_and_carbs_never_below_zero() {
    let api = Api::start().await;
    api.profile(json!({"height_cm": 152, "sex": "female", "birth_date": "1946-01-15"}))
        .await;
    api.weigh("2026-03-01", 120.0).await;
    api.eat_daily("2026-03-16", "2026-03-29", 1200.0).await;

    let targets = api.targets("2026-04-08").await;

    assert_eq!(targets["estimated_expenditure_kcal"], 1200.0);
    assert_eq!(targets["bmr_kcal"], 1589.0);
    assert_eq!(targets["calorie_target_kcal"], 1589.0);
    assert_eq!(targets["bmr_floor_applied"], true);
    assert_eq!(
        targets["macros"],
        json!({
            "protein": {"target_g": 264.0, "eaten_g": 0.0, "status": "below"},
            "fat": {"floor_g": 84.0, "eaten_g": 0.0, "status": "below"},
            "carbs": {"target_g": 0.0, "eaten_g": 0.0},
        })
    );
}

#[tokio::test]
async fn a_goal_change_applies_from_the_day_it_is_set_and_past_days_keep_their_targets() {
    let api = Api::start().await;
    let today = today();
    let yesterday = today - Days::new(1);
    let birthday = today - Days::new(120);
    let birth_date = birthday.with_year(birthday.year() - 30).unwrap();
    api.profile(json!({"height_cm": 180, "sex": "male", "birth_date": birth_date.to_string()}))
        .await;
    api.weigh(&(today - Days::new(20)).to_string(), 80.0).await;

    let goal = api.goal().await;
    assert_eq!(
        goal,
        json!({
            "goal": {"direction": "lose", "rate_kg_per_week": 0.5, "effective_from": null},
            "history": [],
        })
    );
    let before = api.targets(&yesterday.to_string()).await;
    assert_eq!(before["calorie_target_kcal"], 2120.0);

    let (status, body) = api
        .set_goal(json!({"direction": "maintain", "rate_kg_per_week": 0}))
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let maintain = json!({
        "direction": "maintain",
        "rate_kg_per_week": 0.0,
        "effective_from": today.to_string(),
    });
    assert_eq!(body["goal"], maintain);
    assert_eq!(body["history"], json!([maintain]));

    let now = api.targets(&today.to_string()).await;
    assert_eq!(now["goal"], maintain);
    assert_eq!(now["estimated_expenditure_kcal"], 2670.0);
    assert_eq!(now["calorie_target_kcal"], 2670.0);
    assert_eq!(api.targets(&yesterday.to_string()).await, before);

    let (status, body) = api
        .set_goal(json!({"direction": "lean_bulk", "rate_kg_per_week": 0.25}))
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["history"].as_array().unwrap().len(), 1);
    let bulk = api.targets(&today.to_string()).await;
    assert_eq!(bulk["calorie_target_kcal"], 2945.0);
    assert_eq!(api.goal().await["goal"]["direction"], "lean_bulk");
}

#[tokio::test]
async fn only_the_offered_goals_can_be_set() {
    let api = Api::start().await;

    for invalid in [
        json!({"direction": "lose", "rate_kg_per_week": 0.3}),
        json!({"direction": "lose", "rate_kg_per_week": 1.0}),
        json!({"direction": "lean_bulk", "rate_kg_per_week": 0.5}),
        json!({"direction": "maintain", "rate_kg_per_week": 0.25}),
        json!({"direction": "cut", "rate_kg_per_week": 0.5}),
        json!({"direction": "lose"}),
        json!({"direction": "lose", "rate_kg_per_week": 0.5, "extra": true}),
    ] {
        let (status, body) = api.set_goal(invalid.clone()).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{invalid}: {body}");
    }
    assert_eq!(api.goal().await["history"], json!([]));

    for (direction, rate) in [
        ("lose", 0.25),
        ("lose", 0.75),
        ("maintain", 0.0),
        ("lean_bulk", 0.1),
    ] {
        let (status, body) = api
            .set_goal(json!({"direction": direction, "rate_kg_per_week": rate}))
            .await;
        assert_eq!(status, StatusCode::OK, "{direction} {rate}: {body}");
    }
    let (status, body) = api.set_goal(json!({"direction": "maintain"})).await;
    assert_eq!(status, StatusCode::OK, "{body}");
}
