//! Nutrition targets through the Nutrition Day: Trend Weight, Estimated Expenditure, the weekly
//! Calorie Target and the Macro Targets, from a real `app.db` seeded through the Profile, Body
//! Weight, Nutrition Goal and Meal Log routes, and a real `food.db` imported from a synthetic
//! DTU-shaped workbook served by `wiremock`.
//!
//! Synthetic Chicken breast (Food 907) has 150 kcal, 21 g protein and 7 g fat per 100 g, so a
//! day's intake is logged as `kcal / 1.5` grams of it.

mod common;

use std::sync::atomic::{AtomicUsize, Ordering};

use axum::{Router, http::Method, http::StatusCode};
use chrono::{Datelike, Days, NaiveDate, Utc};
use common::food_workbook::{first_dataset, sha256};
use common::{send_request_with_method, test_app_with_db};
use openhome_api::{
    auth::{ApiKey, auth_middleware},
    db, routes,
    services::nutrition::{
        food_database::{DatasetPin, FoodDatabase},
        meal_photos::MealPhotos,
        open_food_facts::OpenFoodFacts,
    },
};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use tempfile::TempDir;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};

const API_KEY: &str = "test-api-key";

/// The nutrition routes and the Profile and Body Weight routes over one `app.db`.
struct Api {
    _server: MockServer,
    _dir: TempDir,
    nutrition: Router,
    fitness: Router,
    logs: AtomicUsize,
}

impl Api {
    async fn start() -> Self {
        let server = MockServer::start().await;
        let dir = TempDir::new().unwrap();
        let (fitness, state) = test_app_with_db().await;
        let bytes = first_dataset();
        Mock::given(method("GET"))
            .and(path("/files/1.xlsx"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(bytes.clone()))
            .mount(&server)
            .await;
        let pin = DatasetPin {
            version: "1".into(),
            url: format!("{}/files/1.xlsx", server.uri()).parse().unwrap(),
            sha256: sha256(&bytes),
        };
        let url = format!("sqlite:{}", dir.path().join("food.db").display());
        let pool = db::connect_sqlite(&url).await.unwrap();
        let (foods, import) = FoodDatabase::open(pool, pin, dir.path().join("food-dataset"))
            .await
            .unwrap();
        import.expect("a new pin imports").run().await;
        let nutrition = routes::nutrition::router(
            foods,
            MealPhotos::new(dir.path().join("meal-photos")),
            OpenFoodFacts::new("http://127.0.0.1:9/".parse().unwrap()).unwrap(),
        )
        .with_state(state)
        .layer(axum::middleware::from_fn(|request, next| {
            auth_middleware(request, next, ApiKey::new(API_KEY.into()))
        }));
        Self {
            _server: server,
            _dir: dir,
            nutrition,
            fitness,
            logs: AtomicUsize::new(0),
        }
    }

    async fn send(&self, method: Method, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
        send_request_with_method(self.nutrition.clone(), uri, method, body, Some(API_KEY)).await
    }

    async fn profile(&self, body: Value) {
        let (status, body) = send_request_with_method(
            self.fitness.clone(),
            "/api/profile",
            Method::PATCH,
            Some(body),
            Some(API_KEY),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
    }

    async fn weigh(&self, date: &str, weight_kg: f64) {
        let (status, body) = send_request_with_method(
            self.fitness.clone(),
            "/api/body_weight",
            Method::POST,
            Some(json!({"date": date, "weight_kg": weight_kg})),
            Some(API_KEY),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
    }

    /// Weighs in at `weight_kg` on every day from `from` through `to`.
    async fn weigh_daily(&self, from: &str, to: &str, weight_kg: f64) {
        for day in days(from, to) {
            self.weigh(&day, weight_kg).await;
        }
    }

    /// Logs `kcal` of Chicken breast at noon on `day`.
    async fn eat(&self, day: &str, kcal: f64) {
        let request_id = format!("meal-{}", self.logs.fetch_add(1, Ordering::Relaxed));
        let (status, body) = self
            .send(
                Method::POST,
                "/api/nutrition/meal-logs",
                Some(json!({
                    "eaten_at": format!("{day}T12:00:00+01:00"),
                    "portions": [{"food_id": 907, "grams": kcal / 1.5}],
                    "request_id": request_id,
                })),
            )
            .await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
    }

    /// Logs `kcal` on every day from `from` through `to`.
    async fn eat_daily(&self, from: &str, to: &str, kcal: f64) {
        for day in days(from, to) {
            self.eat(&day, kcal).await;
        }
    }

    async fn targets(&self, day: &str) -> Value {
        let (status, body) = self
            .send(Method::GET, &format!("/api/nutrition/days/{day}"), None)
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        body["targets"].clone()
    }

    async fn set_goal(&self, body: Value) -> (StatusCode, Value) {
        self.send(Method::PUT, "/api/nutrition/goal", Some(body))
            .await
    }

    async fn goal(&self) -> Value {
        let (status, body) = self.send(Method::GET, "/api/nutrition/goal", None).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        body
    }
}

fn date(text: &str) -> NaiveDate {
    NaiveDate::parse_from_str(text, "%Y-%m-%d").unwrap()
}

fn days(from: &str, to: &str) -> Vec<String> {
    date(from)
        .iter_days()
        .take_while(|day| *day <= date(to))
        .map(|day| day.to_string())
        .collect()
}

/// A 30-year-old man of 180 cm on 2026-04-06; at 80 kg his BMR is 1,780 kcal.
async fn man(api: &Api) {
    api.profile(json!({"height_cm": 180, "sex": "male", "birth_date": "1996-01-15"}))
        .await;
}

/// Today in Copenhagen.
fn today() -> NaiveDate {
    Utc::now()
        .with_timezone(&chrono_tz::Europe::Copenhagen)
        .date_naive()
}

// The week of Monday 2026-04-06 is read in most tests. Its Estimated Expenditure comes from the
// 28 days 2026-03-09 to 2026-04-05, across the change to summer time on 2026-03-29.

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
    // A weigh-in after the day does not give it a Trend Weight.
    api.weigh("2026-04-09", 80.0).await;
    let targets = api.targets("2026-04-08").await;
    assert_eq!(targets["missing"], json!(["body_weight"]));
    assert_eq!(
        targets["reason"],
        "Targets need a Body Weight on or before 2026-04-08"
    );

    // The week's first weigh-in sets its targets when nothing was weighed before Monday.
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

    // 1,000 g of chicken: 1,500 kcal, 210 g protein and 70 g fat.
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
    // Seven logged days in the window; a day before it and a day of the week itself are ignored.
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
    // Half the 2,000 kcal data estimate and half the 2,670 kcal prior.
    assert_eq!(targets["estimated_expenditure_kcal"], 2335.0);
    assert_eq!(targets["calorie_target_kcal"], 1785.0);
}

#[tokio::test]
async fn fourteen_logged_days_make_the_expenditure_intake_corrected_by_the_trend_weight_change() {
    let api = Api::start().await;
    man(&api).await;
    api.weigh("2026-03-01", 80.0).await;
    // Daily 79 kg weigh-ins move the Trend Weight from 80 to 79.0523 kg over the 28 days.
    api.weigh_daily("2026-03-09", "2026-04-05", 79.0).await;
    api.eat_daily("2026-03-12", "2026-03-31", 2500.0).await;

    let targets = api.targets("2026-04-08").await;

    // 2,500 kcal eaten while losing 0.9477 kg in 28 days: 2,500 + 0.9477 × 7,700 ÷ 28.
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

    // Every day of the read week keeps the stored expenditure, training and rest days alike.
    assert_eq!(api.targets("2026-04-08").await, before);
    let sunday = api.targets("2026-04-12").await;
    assert_eq!(sunday["estimated_expenditure_kcal"], 2670.0);
    assert_eq!(sunday["calorie_target_kcal"], 2120.0);
    // The next week, read for the first time, counts the backfilled days in its window
    // (2026-03-16 to 2026-04-12): 8 logged days at 2,000 kcal blended with the 2,670 kcal prior.
    let next = api.targets("2026-04-13").await;
    assert_eq!(next["expenditure_basis"]["logged_days"], 8);
    assert_eq!(next["estimated_expenditure_kcal"], 2287.0);
    // 2,287 - 550 kcal would fall below the 1,780 kcal BMR.
    assert_eq!(next["calorie_target_kcal"], 1780.0);
}

#[tokio::test]
async fn the_calorie_target_never_falls_below_bmr_and_carbs_never_below_zero() {
    let api = Api::start().await;
    // An 80-year-old woman of 152 cm at 120 kg: BMR 1,589 kcal.
    api.profile(json!({"height_cm": 152, "sex": "female", "birth_date": "1946-01-15"}))
        .await;
    api.weigh("2026-03-01", 120.0).await;
    api.eat_daily("2026-03-16", "2026-03-29", 1200.0).await;

    let targets = api.targets("2026-04-08").await;

    assert_eq!(targets["estimated_expenditure_kcal"], 1200.0);
    assert_eq!(targets["bmr_kcal"], 1589.0);
    assert_eq!(targets["calorie_target_kcal"], 1589.0);
    assert_eq!(targets["bmr_floor_applied"], true);
    // 264 g protein and an 84 g fat floor already exceed the target's energy.
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
    // 30 years old on both days' Mondays: the 30th birthday was 120 days ago.
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

    // A second change on the same day replaces the first.
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
    // Maintain needs no rate.
    let (status, body) = api.set_goal(json!({"direction": "maintain"})).await;
    assert_eq!(status, StatusCode::OK, "{body}");
}
