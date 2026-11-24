use crate::builder::MACRO;
use crate::builder::SyntheticFood;
use crate::builder::parameter;
use crate::builder::sha256;
use crate::builder::workbook;
use crate::send_request_with_method::send_request_with_method;
use crate::test_app_with_db::test_app_with_db;
use axum::{Router, http::Method, http::StatusCode};
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

pub(super) const VITAMINS: (&str, &str) = ("Vitaminer", "Vitamins");
pub(super) const MINERALS: (&str, &str) = (
    "Mineraler og uorganiske stoffer",
    "Minerals and inorganic substances",
);
pub(super) const PUFA: (&str, &str) = ("Flerumættede fedtsyrer", "Polyunsaturated fatty acids");

pub(super) fn dataset() -> Vec<u8> {
    let parameters = [
        parameter(
            356,
            ("Energi (kcal)", "Energy (kcal)"),
            "kcal/100 g",
            1030,
            MACRO,
        ),
        parameter(
            12,
            ("A-vitamin", "Vitamin A"),
            "RE (µg/100g)",
            2010,
            VITAMINS,
        ),
        parameter(225, ("Retinol", "Retinol"), "µg/100g", 2020, VITAMINS),
        parameter(126, ("D-vitamin", "Vitamin D"), "µg/100g", 2110, VITAMINS),
        parameter(
            37,
            ("Thiamin (B1-vitamin)", "Thiamin (Vitamin B1)"),
            "mg/100g",
            2410,
            VITAMINS,
        ),
        parameter(
            203,
            ("Niacinækvivalent", "Niacin equivalent"),
            "NE",
            2430,
            VITAMINS,
        ),
        parameter(143, ("Folat", "Folate"), "µg/100g", 2470, VITAMINS),
        parameter(47, ("C-vitamin", "Vitamin C"), "mg/100g", 2510, VITAMINS),
        parameter(184, ("Magnesium", "Magnesium"), "mg/100g", 3140, MINERALS),
        parameter(87, ("C20:5,n-3", "C20:5,n-3"), "g/100g", 9110, PUFA),
        parameter(99, ("C22:6,n-3", "C22:6,n-3"), "g/100g", 9120, PUFA),
    ];
    let food = |id, name: &'static str, values: Vec<(i64, f64)>| SyntheticFood {
        id,
        name_da: name,
        name_en: name,
        group: (1, "Syntetisk", "Synthetic"),
        values,
    };
    workbook(
        &parameters,
        &[
            food(1001, "Orange", vec![(356, 47.0), (47, 50.0), (143, 30.0)]),
            food(
                1002,
                "Salmon",
                vec![(356, 200.0), (126, 10.0), (87, 0.5), (99, 1.2)],
            ),
            food(
                1003,
                "Pumpkin seeds",
                vec![(356, 560.0), (184, 600.0), (203, 10.0)],
            ),
            food(
                1004,
                "Liver pate",
                vec![(356, 300.0), (12, 4000.0), (225, 4000.0)],
            ),
            food(
                1005,
                "Wholemeal bread",
                vec![(356, 250.0), (37, 0.3), (143, 300.0)],
            ),
        ],
    )
}

pub(super) struct Api {
    pub(super) _server: MockServer,
    pub(super) _dir: TempDir,
    pub(super) nutrition: Router,
    pub(super) fitness: Router,
    pub(super) requests: AtomicUsize,
}

impl Api {
    pub(super) async fn start() -> Self {
        let server = MockServer::start().await;
        let dir = TempDir::new().unwrap();
        let (fitness, state) = test_app_with_db().await;
        let bytes = dataset();
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
            requests: AtomicUsize::new(0),
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

    pub(super) fn request_id(&self) -> String {
        format!("request-{}", self.requests.fetch_add(1, Ordering::Relaxed))
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

    pub(super) async fn supplement(&self, name: &str, label: Value) -> String {
        let mut body = json!({
            "name": name,
            "dose_unit": "capsule",
            "request_id": self.request_id(),
        });
        body.as_object_mut()
            .unwrap()
            .extend(label.as_object().unwrap().clone());
        let (status, body) = self
            .send(Method::POST, "/api/nutrition/supplements", Some(body))
            .await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
        body["supplement"]["id"].as_str().unwrap().to_owned()
    }

    pub(super) async fn eat(&self, day: &str, portions: &[(i64, f64)], doses: &[(&str, f64)]) {
        let portions: Vec<Value> = portions
            .iter()
            .map(|(food_id, grams)| json!({"food_id": food_id, "grams": grams}))
            .collect();
        let doses: Vec<Value> = doses
            .iter()
            .map(|(supplement_id, doses)| json!({"supplement_id": supplement_id, "doses": doses}))
            .collect();
        let (status, body) = self
            .send(
                Method::POST,
                "/api/nutrition/meal-logs",
                Some(json!({
                    "eaten_at": format!("{day}T12:00:00+02:00"),
                    "portions": portions,
                    "supplement_doses": doses,
                    "request_id": self.request_id(),
                })),
            )
            .await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
    }

    pub(super) async fn day(&self, day: &str) -> Value {
        let (status, body) = self
            .send(Method::GET, &format!("/api/nutrition/days/{day}"), None)
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        body
    }

    pub(super) async fn panel(&self, day: &str) -> Value {
        self.day(day).await["micronutrients"].clone()
    }

    pub(super) async fn nutrient(&self, day: &str, key: &str) -> Value {
        nutrient(&self.panel(day).await, key)
    }

    pub(super) async fn summary(&self, from: &str, to: &str, aggregation: &str) -> Value {
        let (status, body) = self
            .send(
                Method::GET,
                &format!("/api/nutrition/summary?from={from}&to={to}&aggregation={aggregation}"),
                None,
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        body
    }
}

pub(super) fn nutrient(panel: &Value, key: &str) -> Value {
    panel["nutrients"]
        .as_array()
        .unwrap_or_else(|| panic!("no nutrients in {panel}"))
        .iter()
        .find(|nutrient| nutrient["key"] == key)
        .unwrap_or_else(|| panic!("no {key} in {panel}"))
        .clone()
}

pub(super) fn reference(nutrient: &Value) -> (Value, Value) {
    (
        nutrient["recommended_intake"].clone(),
        nutrient["adequate_intake"].clone(),
    )
}

pub(super) async fn woman(api: &Api) {
    api.profile(json!({"sex": "female", "birth_date": "1996-01-15"}))
        .await;
}

pub(super) fn coverage(nutrient: &Value) -> Value {
    json!([
        nutrient["amount"],
        nutrient["food"],
        nutrient["supplements"],
        nutrient["percent_of_recommended"],
    ])
}

pub(super) fn limits(nutrient: &Value) -> Value {
    nutrient["upper_limits"]
        .as_array()
        .unwrap()
        .iter()
        .map(|limit| {
            json!([
                limit["applies_to"],
                limit["limit"],
                limit["amount"],
                limit["seven_day_average"],
                limit["averaged_days"],
                limit["status"],
            ])
        })
        .collect()
}

pub(super) fn period_coverage(coverage: &Value, key: &str) -> Value {
    let nutrient = nutrient(coverage, key);
    json!([
        nutrient["percent_of_recommended"],
        nutrient["averaged_days"],
        nutrient["days_above_upper_limit"],
        nutrient["seven_day_average_above_upper_limit"],
    ])
}

pub(super) fn limit_flags(coverage: &Value, key: &str) -> Value {
    let nutrient = nutrient(coverage, key);
    json!([
        nutrient["days_above_upper_limit"],
        nutrient["seven_day_average_above_upper_limit"],
    ])
}
