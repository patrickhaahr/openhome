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
pub(super) const SKYR: &str = "2001234567893";
pub(super) const UNLINKED: &str = "2009876543213";

pub(super) struct Api {
    pub(super) server: MockServer,
    pub(super) dir: TempDir,
    pub(super) state: AppState,
    pub(super) app: Router,
}

impl Api {
    pub(super) async fn start() -> Self {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path(format!("/api/v2/product/{SKYR}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "code": SKYR,
                "status": 1,
                "product": {
                    "code": SKYR,
                    "product_name": "Vanilla Skyr",
                    "brands": "Synthetic Dairy",
                    "nova_group": 3,
                    "nutriments": {
                        "energy-kcal_100g": 80, "proteins_100g": 10,
                        "carbohydrates_100g": 8, "fat_100g": 0.5,
                    },
                },
            })))
            .mount(&server)
            .await;
        let dir = TempDir::new().unwrap();
        let (_, state) = test_app_with_db().await;
        let foods = import(&server, &dir, "1", first_dataset()).await;
        let app = app(&server, &dir, foods, state.clone());
        Self {
            server,
            dir,
            state,
            app,
        }
    }

    pub(super) async fn reimport(&mut self, version: &str, bytes: Vec<u8>) {
        let foods = import(&self.server, &self.dir, version, bytes).await;
        self.app = app(&self.server, &self.dir, foods, self.state.clone());
    }

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
        let app = app(&self.server, &self.dir, foods, self.state.clone());
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

    pub(super) async fn save(&self, body: Value) -> (StatusCode, Value) {
        self.send(Method::POST, "/api/nutrition/meals", Some(body))
            .await
    }

    pub(super) async fn saved(&self, body: Value) -> Value {
        let (status, saved) = self.save(body).await;
        assert_eq!(status, StatusCode::CREATED, "{saved}");
        assert_eq!(saved["replayed"], false);
        saved["meal"].clone()
    }

    pub(super) async fn update(&self, id: &Value, body: Value) -> (StatusCode, Value) {
        self.send(
            Method::PATCH,
            &format!("/api/nutrition/meals/{}", id.as_str().unwrap()),
            Some(body),
        )
        .await
    }

    pub(super) async fn delete(&self, id: &Value) -> StatusCode {
        self.send(
            Method::DELETE,
            &format!("/api/nutrition/meals/{}", id.as_str().unwrap()),
            None,
        )
        .await
        .0
    }

    pub(super) async fn meals(&self) -> Vec<Value> {
        let (status, body) = self.send(Method::GET, "/api/nutrition/meals", None).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        body["meals"].as_array().unwrap().clone()
    }

    pub(super) async fn log(&self, body: Value) -> (StatusCode, Value) {
        self.send(Method::POST, "/api/nutrition/meal-logs", Some(body))
            .await
    }

    pub(super) async fn logged(&self, body: Value) -> Value {
        let (status, logged) = self.log(body).await;
        assert_eq!(status, StatusCode::CREATED, "{logged}");
        assert_eq!(logged["replayed"], false);
        logged["meal_log"].clone()
    }

    pub(super) async fn day(&self, day: &str) -> Value {
        let (status, body) = self
            .send(Method::GET, &format!("/api/nutrition/days/{day}"), None)
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        body
    }

    pub(super) async fn link(&self, barcode: &str, food_id: i64, request_id: &str) {
        let (status, body) = self
            .send(
                Method::POST,
                "/api/nutrition/products",
                Some(json!({"barcode": barcode, "food_id": food_id, "request_id": request_id})),
            )
            .await;
        assert!(status.is_success(), "{body}");
    }
}

pub(super) async fn open(dir: &TempDir, pin: DatasetPin) -> (FoodDatabase, Option<FoodImport>) {
    let url = format!("sqlite:{}", dir.path().join("food.db").display());
    let pool = db::connect_sqlite(&url).await.unwrap();
    FoodDatabase::open(pool, pin, dir.path().join("food-dataset"))
        .await
        .unwrap()
}

pub(super) async fn import(
    server: &MockServer,
    dir: &TempDir,
    version: &str,
    bytes: Vec<u8>,
) -> FoodDatabase {
    Mock::given(method("GET"))
        .and(path(format!("/files/{version}.xlsx")))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(bytes.clone()))
        .mount(server)
        .await;
    let pin = DatasetPin {
        version: version.into(),
        url: format!("{}/files/{version}.xlsx", server.uri())
            .parse()
            .unwrap(),
        sha256: sha256(&bytes),
    };
    let (foods, import) = open(dir, pin).await;
    import.expect("a new pin imports").run().await;
    foods
}

pub(super) fn app(
    server: &MockServer,
    dir: &TempDir,
    foods: FoodDatabase,
    state: AppState,
) -> Router {
    let labels = OpenFoodFacts::new(format!("{}/", server.uri()).parse().unwrap()).unwrap();
    routes::nutrition::router(
        foods,
        MealPhotos::new(dir.path().join("meal-photos")),
        labels,
    )
    .with_state(state)
    .layer(axum::middleware::from_fn(|request, next| {
        auth_middleware(request, next, ApiKey::new(API_KEY.into()))
    }))
}

pub(super) fn protein_bowl(request_id: &str) -> Value {
    json!({
        "name": "  Protein   bowl ",
        "portions": [{"food_id": 1693, "grams": 150}, {"food_id": 907, "grams": 120}],
        "request_id": request_id,
    })
}

pub(super) fn by_name(name: &str, request_id: &str) -> Value {
    json!({
        "eaten_at": "2026-10-29T08:15:00+01:00",
        "meal": name,
        "request_id": request_id,
    })
}

pub(super) fn product_at(barcode: &str, eaten_at: &str, request_id: &str) -> Value {
    json!({
        "eaten_at": format!("2026-10-29T{eaten_at}+01:00"),
        "portions": [{"barcode": barcode, "grams": 150}],
        "request_id": request_id,
    })
}
