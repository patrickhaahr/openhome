// Each test binary uses a subset of the shared helpers.
#![allow(dead_code)]

use crate::{
    common::food_workbook::first_dataset, common::food_workbook::sha256,
    common::send_request_with_method, common::test_app_with_db,
};
use axum::{Router, http::Method, http::StatusCode};
use openhome_api::{
    AppState, auth::ApiKey, auth::auth_middleware, db, routes,
    services::nutrition::food_database::DatasetPin,
    services::nutrition::food_database::FoodDatabase,
    services::nutrition::food_database::FoodImport, services::nutrition::meal_photos::MealPhotos,
    services::nutrition::open_food_facts::OpenFoodFacts,
};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use tempfile::TempDir;
use wiremock::{Mock, MockServer, ResponseTemplate, matchers::method, matchers::path};

pub(super) const API_KEY: &str = "test-api-key";

/// The nutrition routes over one `app.db` and a `food.db` imported from `wiremock`.
pub(super) struct Api {
    pub(super) server: MockServer,
    pub(super) dir: TempDir,
    pub(super) state: AppState,
    pub(super) app: Router,
}

impl Api {
    pub(super) async fn start() -> Self {
        let server = MockServer::start().await;
        let dir = TempDir::new().unwrap();
        let (_, state) = test_app_with_db().await;
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
        let (foods, import) = open(&dir, pin).await;
        import.expect("a new pin imports").run().await;
        let app = app(&dir, foods, state.clone());
        Self {
            server,
            dir,
            state,
            app,
        }
    }

    /// The same data while a new pin is still being imported.
    pub(super) async fn importing(self) -> Self {
        let pin = DatasetPin {
            version: "never".into(),
            url: format!("{}/files/never.xlsx", self.server.uri())
                .parse()
                .unwrap(),
            sha256: sha256(b"never served"),
        };
        let (foods, import) = open(&self.dir, pin).await;
        assert!(import.is_some());
        let app = app(&self.dir, foods, self.state.clone());
        Self { app, ..self }
    }

    pub(super) async fn send(
        &self,
        method: Method,
        uri: &str,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        send_request_with_method(self.app.clone(), uri, method, body, Some(API_KEY)).await
    }

    pub(super) async fn add(&self, body: Value) -> (StatusCode, Value) {
        self.send(Method::POST, "/api/nutrition/supplements", Some(body))
            .await
    }

    /// Adds `body`, expecting a new Supplement, and returns it.
    pub(super) async fn added(&self, body: Value) -> Value {
        let (status, added) = self.add(body).await;
        assert_eq!(status, StatusCode::CREATED, "{added}");
        assert_eq!(added["replayed"], false);
        added["supplement"].clone()
    }

    pub(super) async fn update(&self, id: &Value, body: Value) -> (StatusCode, Value) {
        self.send(
            Method::PATCH,
            &format!("/api/nutrition/supplements/{}", id.as_str().unwrap()),
            Some(body),
        )
        .await
    }

    pub(super) async fn delete(&self, id: &Value) -> (StatusCode, Value) {
        self.send(
            Method::DELETE,
            &format!("/api/nutrition/supplements/{}", id.as_str().unwrap()),
            None,
        )
        .await
    }

    pub(super) async fn log(&self, body: Value) -> (StatusCode, Value) {
        self.send(Method::POST, "/api/nutrition/meal-logs", Some(body))
            .await
    }

    /// Logs `body`, expecting a new Meal Log, and returns it.
    pub(super) async fn logged(&self, body: Value) -> Value {
        let (status, logged) = self.log(body).await;
        assert_eq!(status, StatusCode::CREATED, "{logged}");
        assert_eq!(logged["replayed"], false);
        logged["meal_log"].clone()
    }

    pub(super) async fn update_log(&self, id: &Value, body: Value) -> (StatusCode, Value) {
        self.send(
            Method::PATCH,
            &format!("/api/nutrition/meal-logs/{}", id.as_str().unwrap()),
            Some(body),
        )
        .await
    }

    pub(super) async fn meal_logs(&self, day: &str) -> Vec<Value> {
        let (status, body) = self
            .send(
                Method::GET,
                &format!("/api/nutrition/meal-logs?day={day}"),
                None,
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        body["meal_logs"].as_array().unwrap().clone()
    }

    pub(super) async fn day(&self, day: &str) -> Value {
        let (status, body) = self
            .send(Method::GET, &format!("/api/nutrition/days/{day}"), None)
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        body
    }

    /// The day's total of the nutrient `key`.
    pub(super) async fn nutrient(&self, day: &str, key: &str) -> Value {
        let day = self.day(day).await;
        day["nutrients"]
            .as_array()
            .unwrap()
            .iter()
            .find(|nutrient| nutrient["key"] == key)
            .cloned()
            .unwrap_or_else(|| panic!("{key} is missing from {day}"))
    }

    pub(super) async fn save_meal(&self, body: Value) -> (StatusCode, Value) {
        self.send(Method::POST, "/api/nutrition/meals", Some(body))
            .await
    }

    pub(super) async fn update_meal(&self, id: &Value, body: Value) -> (StatusCode, Value) {
        self.send(
            Method::PATCH,
            &format!("/api/nutrition/meals/{}", id.as_str().unwrap()),
            Some(body),
        )
        .await
    }

    pub(super) async fn delete_meal(&self, id: &Value) -> StatusCode {
        self.send(
            Method::DELETE,
            &format!("/api/nutrition/meals/{}", id.as_str().unwrap()),
            None,
        )
        .await
        .0
    }

    pub(super) async fn supplements(&self) -> Vec<Value> {
        let (status, body) = self
            .send(Method::GET, "/api/nutrition/supplements", None)
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        body["supplements"].as_array().unwrap().clone()
    }
}

pub(super) async fn open(dir: &TempDir, pin: DatasetPin) -> (FoodDatabase, Option<FoodImport>) {
    let url = format!("sqlite:{}", dir.path().join("food.db").display());
    let pool = db::connect_sqlite(&url).await.unwrap();
    FoodDatabase::open(pool, pin, dir.path().join("food-dataset"))
        .await
        .unwrap()
}

pub(super) fn app(dir: &TempDir, foods: FoodDatabase, state: AppState) -> Router {
    routes::nutrition::router(
        foods,
        MealPhotos::new(dir.path().join("meal-photos")),
        OpenFoodFacts::new("http://127.0.0.1:9/".parse().unwrap()).unwrap(),
    )
    .with_state(state)
    .layer(axum::middleware::from_fn(|request, next| {
        auth_middleware(request, next, ApiKey::new(API_KEY.into()))
    }))
}

/// A fish oil capsule as its label states it: omega-3 in mg, vitamin D in µg and its energy.
pub(super) fn fish_oil(request_id: &str) -> Value {
    json!({
        "name": "  Omega-3   fish oil ",
        "dose_unit": "capsule",
        "amounts": [
            {"key": "epa", "value": 180, "unit": "mg"},
            {"key": "dha", "value": 120, "unit": "mg"},
            {"key": "vitamin_d", "value": 5, "unit": "µg"},
            {"key": "energy_kcal", "value": 9, "unit": "kcal"},
            {"key": "fat", "value": 1, "unit": "g"},
        ],
        "request_id": request_id,
    })
}

/// One scoop of creatine: a compound without a database parameter.
pub(super) fn creatine(request_id: &str) -> Value {
    json!({
        "name": "Creatine",
        "dose_unit": "scoop",
        "compounds": [{"name": "Creatine  monohydrate", "value": 3, "unit": "g"}],
        "request_id": request_id,
    })
}

/// Breakfast on 2026-10-29: chicken with the doses given.
pub(super) fn breakfast(request_id: &str, doses: &Value) -> Value {
    json!({
        "eaten_at": "2026-10-29T08:15:00+01:00",
        "portions": [{"food_id": 907, "grams": 200}],
        "supplement_doses": doses,
        "request_id": request_id,
    })
}
