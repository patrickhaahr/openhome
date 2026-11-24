use crate::builder::sha256;
use crate::common::{send_request_with_method, test_app_with_db};
use crate::dataset_v1::first_dataset;
use axum::{Router, http::Method, http::StatusCode};
use openhome_api::{
    AppState, auth::ApiKey, auth::auth_middleware, db, routes,
    services::nutrition::food_database::DatasetPin,
    services::nutrition::food_database::FoodDatabase, services::nutrition::meal_photos::MealPhotos,
    services::nutrition::open_food_facts::OpenFoodFacts,
};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use tempfile::TempDir;
use wiremock::{Mock, MockServer, Request, ResponseTemplate, matchers::method, matchers::path};

pub(super) const API_KEY: &str = "test-api-key";
pub(super) const SKYR: &str = "2001234567893";
pub(super) const JUICE: &str = "2009876543213";
pub(super) const UNKNOWN: &str = "2005550001116";

pub(super) struct Api {
    pub(super) server: MockServer,
    pub(super) _dir: TempDir,
    pub(super) state: AppState,
    pub(super) app: Router,
}

impl Api {
    pub(super) async fn start() -> Self {
        let server = MockServer::start().await;
        let dir = TempDir::new().unwrap();
        let workbook = first_dataset();
        Mock::given(method("GET"))
            .and(path("/files/1.xlsx"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(workbook.clone()))
            .mount(&server)
            .await;
        let pin = DatasetPin {
            version: "1".into(),
            url: format!("{}/files/1.xlsx", server.uri()).parse().unwrap(),
            sha256: sha256(&workbook),
        };
        let url = format!("sqlite:{}", dir.path().join("food.db").display());
        let pool = db::connect_sqlite(&url).await.unwrap();
        let (foods, import) = FoodDatabase::open(pool, pin, dir.path().join("food-dataset"))
            .await
            .unwrap();
        import.expect("a new pin imports").run().await;
        let (_, state) = test_app_with_db().await;
        let labels = OpenFoodFacts::new(format!("{}/", server.uri()).parse().unwrap()).unwrap();
        let app = routes::nutrition::router(
            foods,
            MealPhotos::new(dir.path().join("meal-photos")),
            labels,
        )
        .with_state(state.clone())
        .layer(axum::middleware::from_fn(|request, next| {
            auth_middleware(request, next, ApiKey::new(API_KEY.into()))
        }));
        Self {
            server,
            _dir: dir,
            state,
            app,
        }
    }

    pub(super) async fn send(
        &self,
        method: Method,
        uri: &str,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        send_request_with_method(self.app.clone(), uri, method, body, Some(API_KEY)).await
    }

    pub(super) async fn lookup(&self, barcode: &str) -> (StatusCode, Value) {
        self.send(
            Method::GET,
            &format!("/api/nutrition/products/{barcode}"),
            None,
        )
        .await
    }

    pub(super) async fn link(
        &self,
        barcode: &str,
        food_id: i64,
        request_id: &str,
    ) -> (StatusCode, Value) {
        self.send(
            Method::POST,
            "/api/nutrition/products",
            Some(json!({"barcode": barcode, "food_id": food_id, "request_id": request_id})),
        )
        .await
    }

    pub(super) async fn provider(&self, barcode: &str, template: ResponseTemplate, reads: u64) {
        Mock::given(method("GET"))
            .and(path(format!("/api/v2/product/{barcode}")))
            .and(identified_and_limited)
            .respond_with(template)
            .expect(reads)
            .mount(&self.server)
            .await;
    }

    pub(super) async fn provider_reads(&self) -> usize {
        self.server
            .received_requests()
            .await
            .unwrap()
            .iter()
            .filter(|request| request.url.path().starts_with("/api/v2/product/"))
            .count()
    }
}

pub(super) fn identified_and_limited(request: &Request) -> bool {
    let agent = request
        .headers
        .get("user-agent")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    let fields = request
        .url
        .query_pairs()
        .find(|(name, _)| name == "fields")
        .map(|(_, value)| value.into_owned())
        .unwrap_or_default();
    agent.starts_with("OpenHome/")
        && agent.ends_with(" (+https://github.com/patrickhaahr/openhome)")
        && !request.url.as_str().contains('@')
        && request
            .headers
            .values()
            .all(|value| !value.as_bytes().contains(&b'@'))
        && fields.split(',').any(|field| field == "nutriments")
        && fields.split(',').any(|field| field == "nova_group")
        && !fields.contains('*')
}

pub(super) fn found(product: &Value) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(json!({
        "code": product["code"],
        "product": product,
        "status": 1,
        "status_verbose": "product found",
    }))
}

pub(super) fn skyr_label() -> Value {
    json!({
        "code": SKYR,
        "product_name": "Vanilla Skyr",
        "brands": "Synthetic Dairy, Synthetic Group",
        "generic_name": "Skyr",
        "categories_tags": ["en:dairies", "en:fermented-milk-products"],
        "nova_group": 3,
        "nutriments": {
            "energy-kcal_100g": 80, "energy-kcal_unit": "kcal",
            "proteins_100g": 10, "proteins_serving": 15,
            "carbohydrates_100g": 8, "sugars_100g": 7.5,
            "fat_100g": 0.5, "saturated-fat_100g": 0.3,
            "salt_100g": 0.1, "sodium_100g": 0.04,
            "vitamin-d_100g": 0.0000015, "calcium_100g": 0.12,
        },
    })
}

pub(super) fn label(key: &str, value: f64, unit: &str) -> Value {
    json!({"key": key, "value": value, "unit": unit})
}

impl Api {
    pub(super) async fn log(&self, body: Value) -> (StatusCode, Value) {
        self.send(Method::POST, "/api/nutrition/meal-logs", Some(body))
            .await
    }

    pub(super) async fn logged(&self, portions: Value, request_id: &str) -> Value {
        let (status, logged) = self.log(meal(&portions, request_id)).await;
        assert_eq!(status, StatusCode::CREATED, "{logged}");
        logged["meal_log"].clone()
    }

    pub(super) async fn day(&self) -> Value {
        let (status, day) = self
            .send(Method::GET, "/api/nutrition/days/2026-10-29", None)
            .await;
        assert_eq!(status, StatusCode::OK, "{day}");
        day
    }

    pub(super) async fn linked_skyr(&self, food_id: i64) {
        self.provider(SKYR, found(&skyr_label()), 1).await;
        let (status, linked) = self.link(SKYR, food_id, "link-skyr").await;
        assert_eq!(status, StatusCode::CREATED, "{linked}");
    }
}

pub(super) fn meal(portions: &Value, request_id: &str) -> Value {
    json!({
        "eaten_at": "2026-10-29T08:15:00+01:00",
        "portions": portions,
        "request_id": request_id,
    })
}

pub(super) fn amount(value: Option<f64>, unit: &str, source: Option<&str>) -> Value {
    json!({"value": value, "unit": unit, "source": source})
}

pub(super) fn nutrient<'a>(day: &'a Value, key: &str) -> &'a Value {
    day["nutrients"]
        .as_array()
        .unwrap()
        .iter()
        .find(|nutrient| nutrient["key"] == key)
        .unwrap_or_else(|| panic!("{key} is missing from {day}"))
}
