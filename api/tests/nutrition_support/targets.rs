use crate::builder::sha256;
use crate::common::{send_request_with_method, test_app_with_db, test_app_with_pool};
use crate::dataset_v1::first_dataset;
use axum::{Router, http::Method, http::StatusCode};
use chrono::{Datelike, Days, NaiveDate, Utc};
use openhome_api::{
    AppState, auth::ApiKey, auth::auth_middleware, db, routes,
    services::nutrition::food_database::DatasetPin,
    services::nutrition::food_database::FoodDatabase, services::nutrition::meal_photos::MealPhotos,
    services::nutrition::open_food_facts::OpenFoodFacts,
};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use sqlx::{Row, SqlitePool};
use std::{sync::atomic::AtomicUsize, sync::atomic::Ordering};
use tempfile::TempDir;
use wiremock::{Mock, MockServer, ResponseTemplate, matchers::method, matchers::path};

pub(super) const API_KEY: &str = "test-api-key";

pub(super) struct Api {
    pub(super) _server: MockServer,
    pub(super) _dir: TempDir,
    pub(super) nutrition: Router,
    pub(super) fitness: Router,
    pub(super) db: SqlitePool,
    pub(super) logs: AtomicUsize,
}

impl Api {
    pub(super) async fn start() -> Self {
        let server = MockServer::start().await;
        let dir = TempDir::new().unwrap();
        let (fitness, state) = test_app_with_db().await;
        Self::finish(server, dir, state.db.clone(), fitness, state).await
    }

    pub(super) async fn start_file() -> Self {
        let server = MockServer::start().await;
        let dir = TempDir::new().unwrap();
        let url = format!("sqlite:{}", dir.path().join("app.db").display());
        let db = db::connect_sqlite(&url).await.unwrap();
        sqlx::migrate!("./migrations").run(&db).await.unwrap();
        let (fitness, state) = test_app_with_pool(db.clone(), None);
        Self::finish(server, dir, db, fitness, state).await
    }

    pub(super) async fn finish(
        server: MockServer,
        dir: TempDir,
        db: SqlitePool,
        fitness: Router,
        state: AppState,
    ) -> Self {
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
            db,
            logs: AtomicUsize::new(0),
        }
    }

    pub(super) async fn send(
        &self,
        method: Method,
        uri: &str,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        send_request_with_method(self.nutrition.clone(), uri, method, body, Some(API_KEY)).await
    }

    pub(super) async fn profile(&self, body: Value) {
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

    pub(super) async fn weigh(&self, date: &str, weight_kg: f64) {
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

    pub(super) async fn weigh_daily(&self, from: &str, to: &str, weight_kg: f64) {
        for day in days(from, to) {
            self.weigh(&day, weight_kg).await;
        }
    }

    pub(super) async fn eat(&self, day: &str, kcal: f64) {
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

    pub(super) async fn eat_daily(&self, from: &str, to: &str, kcal: f64) {
        for day in days(from, to) {
            self.eat(&day, kcal).await;
        }
    }

    pub(super) async fn targets(&self, day: &str) -> Value {
        let (status, body) = self
            .send(Method::GET, &format!("/api/nutrition/days/{day}"), None)
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        body["targets"].clone()
    }

    pub(super) async fn summary(&self, from: &str, to: &str) -> Value {
        let (status, body) = self
            .send(
                Method::GET,
                &format!("/api/nutrition/summary?from={from}&to={to}&aggregation=day"),
                None,
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        body
    }

    pub(super) async fn set_goal(&self, body: Value) -> (StatusCode, Value) {
        self.send(Method::PUT, "/api/nutrition/goal", Some(body))
            .await
    }

    pub(super) async fn goal(&self) -> Value {
        let (status, body) = self.send(Method::GET, "/api/nutrition/goal", None).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        body
    }

    pub(super) async fn frozen_weeks(&self) -> Vec<String> {
        sqlx::query("SELECT monday FROM nutrition_weeks ORDER BY monday")
            .fetch_all(&self.db)
            .await
            .unwrap()
            .iter()
            .map(|row| row.get::<String, _>(0))
            .collect()
    }

    pub(super) async fn seed_week(&self, monday: &str, trend_weight_day: &str) {
        sqlx::query(
            "INSERT INTO nutrition_weeks (monday, expenditure_kcal, bmr_kcal, prior_kcal,
                data_estimate_kcal, logged_days, blend_weight, trend_weight_kg, trend_weight_day)
            VALUES (?1, 3000, 1600, 2650, NULL, 0, 0.0, 80, ?2)",
        )
        .bind(monday)
        .bind(trend_weight_day)
        .execute(&self.db)
        .await
        .unwrap();
    }
}

pub(super) fn date(text: &str) -> NaiveDate {
    NaiveDate::parse_from_str(text, "%Y-%m-%d").unwrap()
}

pub(super) fn days(from: &str, to: &str) -> Vec<String> {
    date(from)
        .iter_days()
        .take_while(|day| *day <= date(to))
        .map(|day| day.to_string())
        .collect()
}

pub(super) async fn man(api: &Api) {
    api.profile(json!({"height_cm": 180, "sex": "male", "birth_date": "1996-01-15"}))
        .await;
}

pub(super) fn today() -> NaiveDate {
    Utc::now()
        .with_timezone(&chrono_tz::Europe::Copenhagen)
        .date_naive()
}

pub(super) async fn man_of_today(api: &Api) {
    let birthday = (today() - Days::new(120))
        .with_year(today().year() - 30)
        .unwrap();
    api.profile(json!({"height_cm": 180, "sex": "male", "birth_date": birthday.to_string()}))
        .await;
}

pub(super) fn day_from_today(offset: i64) -> String {
    let day = if offset >= 0 {
        today().checked_add_days(Days::new(offset.unsigned_abs()))
    } else {
        today().checked_sub_days(Days::new(offset.unsigned_abs()))
    };
    day.unwrap().to_string()
}

pub(super) fn monday_of(day: NaiveDate) -> NaiveDate {
    day - chrono::Duration::days(i64::from(day.weekday().num_days_from_monday()))
}
