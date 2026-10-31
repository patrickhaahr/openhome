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

use std::sync::atomic::{AtomicUsize, Ordering};

use axum::{Router, http::Method, http::StatusCode};
use chrono::{Days, Utc};
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
const CHICKEN: i64 = 907;
const BANANA: i64 = 3;

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

    async fn fitness(&self, method: Method, uri: &str, body: Value) -> StatusCode {
        let (status, body) =
            send_request_with_method(self.fitness.clone(), uri, method, Some(body), Some(API_KEY))
                .await;
        assert!(status.is_success(), "{status} {body}");
        status
    }

    async fn weigh(&self, date: &str, weight_kg: f64) {
        self.fitness(
            Method::POST,
            "/api/body_weight",
            json!({"date": date, "weight_kg": weight_kg}),
        )
        .await;
    }

    /// Logs one meal at noon on `day` of `(food_id, grams)` Portions.
    async fn eat(&self, day: &str, portions: &[(i64, f64)]) {
        let request_id = format!("meal-{}", self.logs.fetch_add(1, Ordering::Relaxed));
        let portions: Vec<Value> = portions
            .iter()
            .map(|(food_id, grams)| json!({"food_id": food_id, "grams": grams}))
            .collect();
        let (status, body) = self
            .send(
                Method::POST,
                "/api/nutrition/meal-logs",
                Some(json!({
                    "eaten_at": format!("{day}T12:00:00+02:00"),
                    "portions": portions,
                    "request_id": request_id,
                })),
            )
            .await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
    }

    async fn classify(&self, food_id: i64, nova_class: u8) {
        let (status, body) = self
            .send(
                Method::PUT,
                &format!("/api/nutrition/foods/{food_id}/nova-class"),
                Some(json!({"nova_class": nova_class})),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
    }

    async fn try_summary(&self, from: &str, to: &str, aggregation: &str) -> (StatusCode, Value) {
        self.send(
            Method::GET,
            &format!("/api/nutrition/summary?from={from}&to={to}&aggregation={aggregation}"),
            None,
        )
        .await
    }

    async fn summary(&self, from: &str, to: &str, aggregation: &str) -> Value {
        let (status, body) = self.try_summary(from, to, aggregation).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        body
    }

    /// Each period's `field`, in order.
    async fn series(&self, from: &str, to: &str, aggregation: &str, field: &str) -> Vec<Value> {
        let summary = self.summary(from, to, aggregation).await;
        summary["periods"]
            .as_array()
            .unwrap()
            .iter()
            .map(|period| {
                json!({"start": period["start"], "end": period["end"], field: period[field]})
            })
            .collect()
    }
}

/// Today in Copenhagen, `offset` days from now.
fn day_from_today(offset: i64) -> String {
    let today = Utc::now()
        .with_timezone(&chrono_tz::Europe::Copenhagen)
        .date_naive();
    let day = if offset >= 0 {
        today.checked_add_days(Days::new(offset.unsigned_abs()))
    } else {
        today.checked_sub_days(Days::new(offset.unsigned_abs()))
    };
    day.unwrap().to_string()
}

/// Three logged days: Thursday 2 April (1,500 kcal of chicken), Saturday 4 April (900 kcal of
/// chicken and 450 kcal of banana) and Monday 6 April (1,500 kcal of chicken). Chicken is NOVA 1
/// and banana NOVA 3.
async fn three_logged_days(api: &Api) {
    api.eat("2026-04-02", &[(CHICKEN, 1000.0)]).await;
    api.eat("2026-04-04", &[(CHICKEN, 600.0), (BANANA, 500.0)])
        .await;
    api.eat("2026-04-06", &[(CHICKEN, 1000.0)]).await;
    api.classify(CHICKEN, 1).await;
    api.classify(BANANA, 3).await;
}

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
            "carbohydrate_kcal": 200.0,
            "fat_kcal": 504.0,
            "alcohol_kcal": null,
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
