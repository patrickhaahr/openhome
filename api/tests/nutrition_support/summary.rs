#![allow(dead_code)]

use crate::{
    common::food_workbook::first_dataset, common::food_workbook::sha256,
    common::send_request_with_method, common::test_app_with_db,
};
use axum::{Router, http::Method, http::StatusCode};
use chrono::{Days, Utc};
use openhome_api::{
    auth::ApiKey, auth::auth_middleware, db, routes,
    services::nutrition::food_database::DatasetPin,
    services::nutrition::food_database::FoodDatabase, services::nutrition::meal_photos::MealPhotos,
    services::nutrition::open_food_facts::OpenFoodFacts,
};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::{sync::atomic::AtomicUsize, sync::atomic::Ordering};
use tempfile::TempDir;
use wiremock::{Mock, MockServer, ResponseTemplate, matchers::method, matchers::path};

pub(super) const API_KEY: &str = "test-api-key";
pub(super) const CHICKEN: i64 = 907;
pub(super) const BANANA: i64 = 3;

pub(super) struct Api {
    pub(super) _server: MockServer,
    pub(super) _dir: TempDir,
    pub(super) nutrition: Router,
    pub(super) fitness: Router,
    pub(super) logs: AtomicUsize,
}

impl Api {
    pub(super) async fn start() -> Self {
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

    pub(super) async fn send(
        &self,
        method: Method,
        uri: &str,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        send_request_with_method(self.nutrition.clone(), uri, method, body, Some(API_KEY)).await
    }

    pub(super) async fn fitness(&self, method: Method, uri: &str, body: Value) -> StatusCode {
        let (status, body) =
            send_request_with_method(self.fitness.clone(), uri, method, Some(body), Some(API_KEY))
                .await;
        assert!(status.is_success(), "{status} {body}");
        status
    }

    pub(super) async fn weigh(&self, date: &str, weight_kg: f64) {
        self.fitness(
            Method::POST,
            "/api/body_weight",
            json!({"date": date, "weight_kg": weight_kg}),
        )
        .await;
    }

    pub(super) async fn eat(&self, day: &str, portions: &[(i64, f64)]) {
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

    pub(super) async fn classify(&self, food_id: i64, nova_class: u8) {
        let (status, body) = self
            .send(
                Method::PUT,
                &format!("/api/nutrition/foods/{food_id}/nova-class"),
                Some(json!({"nova_class": nova_class})),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
    }

    pub(super) async fn try_summary(
        &self,
        from: &str,
        to: &str,
        aggregation: &str,
    ) -> (StatusCode, Value) {
        self.send(
            Method::GET,
            &format!("/api/nutrition/summary?from={from}&to={to}&aggregation={aggregation}"),
            None,
        )
        .await
    }

    pub(super) async fn summary(&self, from: &str, to: &str, aggregation: &str) -> Value {
        let (status, body) = self.try_summary(from, to, aggregation).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        body
    }

    pub(super) async fn series(
        &self,
        from: &str,
        to: &str,
        aggregation: &str,
        field: &str,
    ) -> Vec<Value> {
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

pub(super) fn day_from_today(offset: i64) -> String {
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

pub(super) async fn three_logged_days(api: &Api) {
    api.eat("2026-04-02", &[(CHICKEN, 1000.0)]).await;
    api.eat("2026-04-04", &[(CHICKEN, 600.0), (BANANA, 500.0)])
        .await;
    api.eat("2026-04-06", &[(CHICKEN, 1000.0)]).await;
    api.classify(CHICKEN, 1).await;
    api.classify(BANANA, 3).await;
}
