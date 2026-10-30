//! Products through the nutrition routes: Open Food Facts mocked with `wiremock`, over a real
//! `app.db` and a real `food.db` imported from a synthetic DTU-shaped workbook.

mod common;

use axum::{Router, http::Method, http::StatusCode};
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
    Mock, MockServer, Request, ResponseTemplate,
    matchers::{method, path},
};

const API_KEY: &str = "test-api-key";
/// Synthetic restricted-circulation GS1 numbers: never a real package.
const SKYR: &str = "2001234567893";
const JUICE: &str = "2009876543213";
const UNKNOWN: &str = "2005550001116";

/// The nutrition routes over one `app.db`, a `food.db` imported from `wiremock`, and Open Food
/// Facts on the same `wiremock` server.
struct Api {
    server: MockServer,
    _dir: TempDir,
    app: Router,
}

impl Api {
    async fn start() -> Self {
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
        .with_state(state)
        .layer(axum::middleware::from_fn(|request, next| {
            auth_middleware(request, next, ApiKey::new(API_KEY.into()))
        }));
        Self {
            server,
            _dir: dir,
            app,
        }
    }

    async fn send(&self, method: Method, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
        send_request_with_method(self.app.clone(), uri, method, body, Some(API_KEY)).await
    }

    async fn lookup(&self, barcode: &str) -> (StatusCode, Value) {
        self.send(
            Method::GET,
            &format!("/api/nutrition/products/{barcode}"),
            None,
        )
        .await
    }

    async fn link(&self, barcode: &str, food_id: i64, request_id: &str) -> (StatusCode, Value) {
        self.send(
            Method::POST,
            "/api/nutrition/products",
            Some(json!({"barcode": barcode, "food_id": food_id, "request_id": request_id})),
        )
        .await
    }

    /// Answers reads of `barcode` with `template`, expecting exactly `reads` of them.
    async fn provider(&self, barcode: &str, template: ResponseTemplate, reads: u64) {
        Mock::given(method("GET"))
            .and(path(format!("/api/v2/product/{barcode}")))
            .and(identified_and_limited)
            .respond_with(template)
            .expect(reads)
            .mount(&self.server)
            .await;
    }

    /// How many product reads reached Open Food Facts.
    async fn provider_reads(&self) -> usize {
        self.server
            .received_requests()
            .await
            .unwrap()
            .iter()
            .filter(|request| request.url.path().starts_with("/api/v2/product/"))
            .count()
    }
}

/// A read that names the app in its User-Agent without any contact detail and asks only for
/// the fields a Product needs.
fn identified_and_limited(request: &Request) -> bool {
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

/// A found product as Open Food Facts version 2 answers it.
fn found(product: &Value) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(json!({
        "code": product["code"],
        "product": product,
        "status": 1,
        "status_verbose": "product found",
    }))
}

/// A synthetic skyr label: kcal only, salt and sodium, vitamin D and calcium in grams per 100 g,
/// per-serving values that must be ignored, and no fibre or alcohol.
fn skyr_label() -> Value {
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

fn label(key: &str, value: f64, unit: &str) -> Value {
    json!({"key": key, "value": value, "unit": unit})
}

#[tokio::test]
async fn a_lookup_previews_normalised_label_values_with_suggested_foods_and_stores_nothing() {
    let api = Api::start().await;
    api.provider(SKYR, found(&skyr_label()), 2).await;

    let (status, preview) = api.lookup(SKYR).await;

    assert_eq!(status, StatusCode::OK, "{preview}");
    assert_eq!(preview["linked"], false);
    assert_eq!(
        preview["product"],
        json!({
            "barcode": SKYR,
            "name": "Vanilla Skyr",
            "brand": "Synthetic Dairy",
            "nova_class": 3,
            // Vocabulary order and units: kJ completed from kcal, micronutrients from grams.
            "label_per_100g": [
                label("energy_kj", 334.72, "kJ"),
                label("energy_kcal", 80.0, "kcal"),
                label("protein", 10.0, "g"),
                label("available_carbohydrate", 8.0, "g"),
                label("fat", 0.5, "g"),
                label("salt_labelling", 0.1, "g"),
                label("vitamin_d", 1.5, "µg"),
                label("sodium", 40.0, "mg"),
                label("calcium", 120.0, "mg"),
                label("sugars", 7.5, "g"),
                label("saturated_fat", 0.3, "g"),
            ],
            "food": null,
        })
    );
    assert_eq!(preview["suggested_foods"][0]["food_id"], 1693, "{preview}");
    assert_eq!(
        preview["source"],
        json!({
            "name": "Open Food Facts",
            "url": "https://world.openfoodfacts.org",
            "credit": "Product data © Open Food Facts contributors",
            "licence": "ODbL",
        })
    );
    // Nothing was stored: the next lookup asks Open Food Facts again.
    let (_, again) = api.lookup(SKYR).await;
    assert_eq!(again["linked"], false);
}

#[tokio::test]
async fn linking_stores_the_product_once_and_a_linked_barcode_skips_open_food_facts() {
    let api = Api::start().await;
    // One read previews the barcode and one links it; nothing after that.
    api.provider(SKYR, found(&skyr_label()), 2).await;
    let (_, preview) = api.lookup(SKYR).await;

    let (status, linked) = api.link(SKYR, 1693, "link-skyr").await;

    assert_eq!(status, StatusCode::CREATED, "{linked}");
    assert_eq!(linked["replayed"], false);
    let mut product = preview["product"].clone();
    product["food"] =
        json!({"food_id": 1693, "name_da": "Skyr, 0.2 % fedt", "name_en": "Skyr, 0.2 % fat"});
    assert_eq!(linked["product"], product);
    let (status, lookup) = api.lookup(SKYR).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        (
            &lookup["linked"],
            &lookup["product"],
            &lookup["suggested_foods"]
        ),
        (&json!(true), &product, &json!([]))
    );

    // A retry replays; the same request_id for another Food conflicts.
    let (status, replay) = api.link(SKYR, 1693, "link-skyr").await;
    assert_eq!(
        (status, &replay["replayed"]),
        (StatusCode::OK, &json!(true))
    );
    assert_eq!(replay["product"], product);
    let (status, _) = api.link(SKYR, 907, "link-skyr").await;
    assert_eq!(status, StatusCode::CONFLICT);

    // Re-linking keeps the label values and needs no read.
    let (status, relinked) = api.link(SKYR, 907, "relink-skyr").await;
    assert_eq!(status, StatusCode::OK, "{relinked}");
    assert_eq!(relinked["replayed"], false);
    assert_eq!(relinked["product"]["food"]["food_id"], 907);
    assert_eq!(
        relinked["product"]["label_per_100g"],
        product["label_per_100g"]
    );
    assert_eq!(api.lookup(SKYR).await.1["product"], relinked["product"]);
    assert_eq!(api.provider_reads().await, 2);
}

#[tokio::test]
async fn energy_salt_and_alcohol_are_normalised_from_whatever_the_label_gives() {
    let api = Api::start().await;
    let juice = json!({
        "code": JUICE,
        "product_name": "Sparkling Apple",
        "nova_group": "4",
        "nutriments": {
            // kJ only, under the generic key; sodium only; alcohol in % vol.
            "energy_100g": 1000, "sodium_100g": 0.2, "alcohol_100g": 5,
            "fiber_100g": "1.2", "iron_100g": 0.0021, "vitamin-a_100g": 0.00006,
            // Implausible or unusable values are unknown, never guessed.
            "sugars_100g": 140, "fat_100g": -1, "proteins_100g": "n/a",
        },
    });
    api.provider(JUICE, found(&juice), 1).await;

    let (status, preview) = api.lookup(JUICE).await;

    assert_eq!(status, StatusCode::OK, "{preview}");
    assert_eq!(
        (
            &preview["product"]["brand"],
            &preview["product"]["nova_class"]
        ),
        (&Value::Null, &json!(4))
    );
    assert_eq!(
        preview["product"]["label_per_100g"],
        json!([
            label("energy_kj", 1000.0, "kJ"),
            label("energy_kcal", 239.006, "kcal"),
            label("dietary_fibre", 1.2, "g"),
            label("alcohol", 3.945, "g"),
            label("salt_labelling", 0.5, "g"),
            label("vitamin_a", 60.0, "µg RE"),
            label("sodium", 200.0, "mg"),
            label("iron", 2.1, "mg"),
        ])
    );
}

#[tokio::test]
async fn unknown_barcodes_are_not_found_and_malformed_barcodes_never_reach_open_food_facts() {
    let api = Api::start().await;
    api.provider(
        UNKNOWN,
        ResponseTemplate::new(404).set_body_json(json!({
            "code": UNKNOWN, "status": 0, "status_verbose": "product not found",
        })),
        2,
    )
    .await;
    // A 12-digit UPC-A is read as its 13-digit GTIN.
    api.provider(
        "0012345678905",
        ResponseTemplate::new(200).set_body_json(json!({"code": "0012345678905", "status": 0})),
        1,
    )
    .await;

    for barcode in [UNKNOWN, "012345678905"] {
        let (status, body) = api.lookup(barcode).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{barcode}: {body}");
        assert_eq!(body["code"], "product_not_found", "{barcode}");
        assert!(
            body["error"].as_str().unwrap().contains("closest Food"),
            "{body}"
        );
    }
    let (status, _) = api.link(UNKNOWN, 1693, "link-unknown").await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    for barcode in [
        "12345",
        "2001234567890",
        "200123456789a",
        "20012345678931",
        "%20",
    ] {
        let (status, body) = api.lookup(barcode).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{barcode}: {body}");
    }
    let (status, _) = api.link("2001234567890", 1693, "link-bad-digit").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(api.provider_reads().await, 3);
}

#[tokio::test]
async fn rate_limits_and_failures_are_unavailable_once_without_upstream_text() {
    let api = Api::start().await;
    let barcodes = [
        "2000000000015",
        "2001111111119",
        "2002222222220",
        "2003333333331",
    ];
    let templates = [
        ResponseTemplate::new(429).set_body_string("upstream detail: slow down"),
        ResponseTemplate::new(500).set_body_string("upstream detail: stack trace"),
        ResponseTemplate::new(200).set_body_string("upstream detail: <html>"),
        found(&json!({"code": barcodes[3], "product_name": "x".repeat(300 * 1024)})),
    ];
    // Each failure is read once per request and never retried; the first is also linked.
    for (index, (barcode, template)) in barcodes.into_iter().zip(templates).enumerate() {
        api.provider(barcode, template, if index == 0 { 2 } else { 1 })
            .await;
    }

    for barcode in barcodes {
        let (status, body) = api.lookup(barcode).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{barcode}: {body}");
        assert_eq!(body["code"], "open_food_facts_unavailable");
        assert!(!body.to_string().contains("upstream"), "{body}");
    }
    let (status, _) = api.link(barcodes[0], 1693, "link-limited").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
}

impl Api {
    async fn log(&self, body: Value) -> (StatusCode, Value) {
        self.send(Method::POST, "/api/nutrition/meal-logs", Some(body))
            .await
    }

    /// Logs `portions` at a fixed time, expecting a new Meal Log, and returns it.
    async fn logged(&self, portions: Value, request_id: &str) -> Value {
        let (status, logged) = self.log(meal(&portions, request_id)).await;
        assert_eq!(status, StatusCode::CREATED, "{logged}");
        logged["meal_log"].clone()
    }

    async fn day(&self) -> Value {
        let (status, day) = self
            .send(Method::GET, "/api/nutrition/days/2026-10-29", None)
            .await;
        assert_eq!(status, StatusCode::OK, "{day}");
        day
    }

    /// Links the synthetic skyr to `food_id`, reading Open Food Facts once.
    async fn linked_skyr(&self, food_id: i64) {
        self.provider(SKYR, found(&skyr_label()), 1).await;
        let (status, linked) = self.link(SKYR, food_id, "link-skyr").await;
        assert_eq!(status, StatusCode::CREATED, "{linked}");
    }
}

fn meal(portions: &Value, request_id: &str) -> Value {
    json!({
        "eaten_at": "2026-10-29T08:15:00+01:00",
        "portions": portions,
        "request_id": request_id,
    })
}

fn amount(value: Option<f64>, unit: &str, source: Option<&str>) -> Value {
    json!({"value": value, "unit": unit, "source": source})
}

fn nutrient<'a>(day: &'a Value, key: &str) -> &'a Value {
    day["nutrients"]
        .as_array()
        .unwrap()
        .iter()
        .find(|nutrient| nutrient["key"] == key)
        .unwrap_or_else(|| panic!("{key} is missing from {day}"))
}

#[tokio::test]
async fn a_product_portion_takes_label_values_first_and_its_linked_food_fills_the_gaps() {
    let api = Api::start().await;
    api.linked_skyr(1693).await;

    let meal_log = api
        .logged(json!([{"barcode": SKYR, "grams": 200}]), "skyr-1")
        .await;

    let portion = &meal_log["portions"][0];
    assert_eq!(
        (
            &portion["food_id"],
            &portion["product"],
            &portion["name_en"],
            &portion["grams"]
        ),
        (
            &json!(1693),
            &json!({"barcode": SKYR, "name": "Vanilla Skyr", "brand": "Synthetic Dairy"}),
            &json!("Skyr, 0.2 % fat"),
            &json!(200.0)
        )
    );
    // The label's energy and macronutrients win; the Food has the fibre and alcohol it lacks.
    assert_eq!(
        portion["nutrients"],
        json!({
            "energy_kcal": amount(Some(160.0), "kcal", Some("label")),
            "protein": amount(Some(20.0), "g", Some("label")),
            "available_carbohydrate": amount(Some(16.0), "g", Some("label")),
            "fat": amount(Some(1.0), "g", Some("label")),
            "dietary_fibre": amount(Some(0.0), "g", Some("food")),
            "alcohol": amount(Some(0.0), "g", Some("food")),
        })
    );
    let day = api.day().await;
    // The label's vitamin D (1.5 µg per 100 g, from grams) wins over the Food's 0 µg, and its
    // kJ comes from the label's kcal rather than the Food's 260 kJ.
    assert_eq!(
        (
            &nutrient(&day, "vitamin_d")["value"],
            &nutrient(&day, "energy_kj")["value"],
            &nutrient(&day, "sodium")["value"],
            &nutrient(&day, "sodium")["unit"],
        ),
        (&json!(3.0), &json!(669.44), &json!(80.0), &json!("mg"))
    );
    assert_eq!(day["label_source"]["licence"], "ODbL");
    assert_eq!(day["meal_logs"], json!([meal_log]));
    assert_eq!(api.provider_reads().await, 1);
}

#[tokio::test]
async fn estimates_fill_only_the_values_no_source_has() {
    let api = Api::start().await;
    api.linked_skyr(1693).await;
    // The chicken has no carbohydrate, fibre or alcohol value; nothing has EPA for the skyr.
    let portions = json!([
        {"food_id": 907, "grams": 200, "estimates": {"dietary_fibre": 0.5, "available_carbohydrate": 1}},
        {"barcode": SKYR, "grams": 100, "estimates": {"epa": 0.002}},
    ]);

    let meal_log = api.logged(portions.clone(), "estimated-1").await;

    let chicken = &meal_log["portions"][0]["nutrients"];
    assert_eq!(
        (
            &chicken["dietary_fibre"],
            &chicken["available_carbohydrate"],
            &chicken["protein"],
            &chicken["alcohol"],
        ),
        (
            &amount(Some(1.0), "g", Some("estimate")),
            &amount(Some(2.0), "g", Some("estimate")),
            &amount(Some(42.0), "g", Some("food")),
            &amount(None, "g", None),
        )
    );
    // The chicken's own EPA (0.01 g per 100 g) and the skyr's estimate add up.
    assert_eq!(nutrient(&api.day().await, "epa")["value"], json!(0.022));
    let (status, replay) = api.log(meal(&portions, "estimated-1")).await;
    assert_eq!((status, &replay["meal_log"]), (StatusCode::OK, &meal_log));
    let mut changed = portions;
    changed[1]["estimates"]["epa"] = json!(0.003);
    assert_eq!(
        api.log(meal(&changed, "estimated-1")).await.0,
        StatusCode::CONFLICT
    );

    for (estimates, refused) in [
        (json!({"protein": 20}), "the Food has protein"),
        (json!({"fat": 1}), "the Food has fat"),
        (json!({"unobtainium": 1}), "unknown key"),
        (json!({"waste": 10}), "not a per-100 g value"),
        (json!({"dietary_fibre": -1}), "negative"),
        (json!({"dietary_fibre": 101}), "more than 100 g in 100 g"),
    ] {
        let (status, body) = api
            .log(meal(
                &json!([{"food_id": 907, "grams": 100, "estimates": estimates}]),
                "refused-estimate",
            ))
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{refused}: {body}");
    }
    let (status, body) = api
        .log(meal(
            &json!([{"barcode": SKYR, "grams": 100, "estimates": {"energy_kcal": 70}}]),
            "refused-label-estimate",
        ))
        .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "the label has energy: {body}"
    );
    assert_eq!(api.day().await["meal_logs"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn relinking_a_product_changes_only_future_portions() {
    let api = Api::start().await;
    api.linked_skyr(1693).await;
    let before = api
        .logged(json!([{"barcode": SKYR, "grams": 100}]), "before")
        .await;

    let (status, _) = api.link(SKYR, 907, "relink-skyr").await;
    assert_eq!(status, StatusCode::OK);
    let after = api
        .logged(json!([{"barcode": SKYR, "grams": 100}]), "after")
        .await;

    // The past Portion keeps the skyr Food's fibre; the new one takes the chicken's gaps.
    let day = api.day().await;
    assert_eq!(day["meal_logs"], json!([before, after]));
    assert_eq!(
        (
            &before["portions"][0]["food_id"],
            &before["portions"][0]["nutrients"]["dietary_fibre"],
        ),
        (&json!(1693), &amount(Some(0.0), "g", Some("food")))
    );
    assert_eq!(
        (
            &after["portions"][0]["food_id"],
            &after["portions"][0]["nutrients"]["dietary_fibre"],
            &after["portions"][0]["nutrients"]["energy_kcal"],
        ),
        (
            &json!(907),
            &amount(None, "g", None),
            &amount(Some(80.0), "kcal", Some("label")),
        )
    );
    // Replacing a past Portion snapshots the Product as it is linked now.
    let id = before["id"].as_str().unwrap();
    let (status, replaced) = api
        .send(
            Method::PATCH,
            &format!("/api/nutrition/meal-logs/{id}"),
            Some(json!({"portions": [{"barcode": SKYR, "grams": 150}]})),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{replaced}");
    assert_eq!(replaced["portions"][0]["food_id"], 907);
    assert_eq!(
        replaced["portions"][0]["nutrients"]["protein"],
        amount(Some(15.0), "g", Some("label"))
    );
    assert_eq!(api.provider_reads().await, 1);
}

#[tokio::test]
async fn unlinked_or_ambiguous_product_portions_are_refused_without_writing() {
    let api = Api::start().await;

    for (portion, expected) in [
        (
            json!({"barcode": UNKNOWN, "grams": 100}),
            StatusCode::NOT_FOUND,
        ),
        (
            json!({"barcode": "2001234567890", "grams": 100}),
            StatusCode::BAD_REQUEST,
        ),
        (
            json!({"barcode": SKYR, "food_id": 1693, "grams": 100}),
            StatusCode::BAD_REQUEST,
        ),
        (json!({"grams": 100}), StatusCode::BAD_REQUEST),
    ] {
        let (status, body) = api.log(meal(&json!([portion]), "refused")).await;
        assert_eq!(status, expected, "{portion}: {body}");
        if status == StatusCode::NOT_FOUND {
            assert!(
                body["error"].as_str().unwrap().contains("link_product"),
                "{body}"
            );
        }
    }
    assert_eq!(api.day().await["meal_logs"], json!([]));
    assert_eq!(api.provider_reads().await, 0);
}

#[tokio::test]
async fn the_nova_1_share_counts_classified_kcal_with_products_taking_the_open_food_facts_class() {
    let api = Api::start().await;
    api.linked_skyr(1693).await;
    let (_, preview) = api.lookup(SKYR).await;
    assert_eq!(preview["product"]["nova_class"], 3);

    // Nothing classified yet: no share rather than zero.
    api.logged(json!([{"food_id": 2, "grams": 100}]), "apple")
        .await;
    assert_eq!(
        api.day().await["nova"],
        json!({
            "nova_1_share": null, "nova_1_kcal": 0.0, "classified_kcal": 0.0,
            "unclassified_portions": 1,
        })
    );

    // Classifying the skyr Food leaves the Product on Open Food Facts' class.
    for (food_id, nova_class) in [(1693, 1), (3, 1), (907, 1)] {
        let (status, body) = api
            .send(
                Method::PUT,
                &format!("/api/nutrition/foods/{food_id}/nova-class"),
                Some(json!({"nova_class": nova_class})),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
    }
    let meal_log = api
        .logged(
            json!([
                {"barcode": SKYR, "grams": 200},
                {"food_id": 3, "grams": 100},
                {"food_id": 907, "grams": 100},
            ]),
            "breakfast",
        )
        .await;
    let classes: Vec<&Value> = meal_log["portions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|portion| &portion["nova_class"])
        .collect();
    assert_eq!(classes, [&json!(3), &json!(1), &json!(1)]);

    // Banana 90 kcal and chicken 150 kcal are Class 1 of 400 classified kcal, beside the
    // 160 kcal Class 3 skyr; the unclassified 50 kcal apple is left out.
    assert_eq!(
        api.day().await["nova"],
        json!({
            "nova_1_share": 0.6, "nova_1_kcal": 240.0, "classified_kcal": 400.0,
            "unclassified_portions": 1,
        })
    );
}

#[tokio::test]
async fn a_product_without_an_open_food_facts_class_stays_unclassified_when_its_food_is_classified()
{
    let api = Api::start().await;
    let mut unclassified = skyr_label();
    unclassified["nova_group"] = Value::Null;
    api.provider(SKYR, found(&unclassified), 1).await;
    let (status, linked) = api.link(SKYR, 1693, "link-skyr").await;
    assert_eq!(status, StatusCode::CREATED, "{linked}");
    api.logged(json!([{"barcode": SKYR, "grams": 100}]), "skyr")
        .await;

    let (status, classified) = api
        .send(
            Method::PUT,
            "/api/nutrition/foods/1693/nova-class",
            Some(json!({"nova_class": 1})),
        )
        .await;

    assert_eq!(status, StatusCode::OK, "{classified}");
    assert_eq!(classified["classified_portions"], 0);
    let later = api
        .logged(json!([{"barcode": SKYR, "grams": 100}]), "skyr-2")
        .await;
    assert_eq!(later["portions"][0]["nova_class"], Value::Null);
    let day = api.day().await;
    assert_eq!(
        day["meal_logs"][0]["portions"][0]["nova_class"],
        Value::Null
    );
    assert_eq!(day["nova"]["nova_1_share"], Value::Null);
}
