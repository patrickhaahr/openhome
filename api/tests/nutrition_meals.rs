//! Meals through the nutrition routes: named, reusable lists of Food and Product items logged
//! again by name, over a real `app.db`, a real `food.db` imported from a synthetic DTU-shaped
//! workbook, and Open Food Facts, both served by `wiremock`.

mod common;

use axum::{Router, http::Method, http::StatusCode};
use common::food_workbook::{first_dataset, second_dataset, sha256};
use common::{send_request_with_method, test_app_with_db};
use openhome_api::{
    AppState,
    auth::{ApiKey, auth_middleware},
    db, routes,
    services::nutrition::{
        food_database::{DatasetPin, FoodDatabase, FoodImport},
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
/// A synthetic restricted-circulation GS1 number: never a real package.
const SKYR: &str = "2001234567893";
/// A valid barcode that is never linked.
const UNLINKED: &str = "2009876543213";

/// The nutrition routes over one `app.db`, a `food.db` imported from `wiremock`, and Open Food
/// Facts on the same `wiremock` server, which knows one synthetic skyr.
struct Api {
    server: MockServer,
    dir: TempDir,
    state: AppState,
    app: Router,
}

impl Api {
    /// Imports the first synthetic dataset as version "1".
    async fn start() -> Self {
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

    /// Imports `bytes` as `version` into the same `food.db`, as a pin change on restart does.
    async fn reimport(&mut self, version: &str, bytes: Vec<u8>) {
        let foods = import(&self.server, &self.dir, version, bytes).await;
        self.app = app(&self.server, &self.dir, foods, self.state.clone());
    }

    /// The same data while a new pin is still being imported.
    async fn importing(self) -> Self {
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

    async fn send(&self, method: Method, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
        send_request_with_method(self.app.clone(), uri, method, body, Some(API_KEY)).await
    }

    async fn save(&self, body: Value) -> (StatusCode, Value) {
        self.send(Method::POST, "/api/nutrition/meals", Some(body))
            .await
    }

    /// Saves `body`, expecting a new Meal, and returns it.
    async fn saved(&self, body: Value) -> Value {
        let (status, saved) = self.save(body).await;
        assert_eq!(status, StatusCode::CREATED, "{saved}");
        assert_eq!(saved["replayed"], false);
        saved["meal"].clone()
    }

    async fn update(&self, id: &Value, body: Value) -> (StatusCode, Value) {
        self.send(
            Method::PATCH,
            &format!("/api/nutrition/meals/{}", id.as_str().unwrap()),
            Some(body),
        )
        .await
    }

    async fn delete(&self, id: &Value) -> StatusCode {
        self.send(
            Method::DELETE,
            &format!("/api/nutrition/meals/{}", id.as_str().unwrap()),
            None,
        )
        .await
        .0
    }

    async fn meals(&self) -> Vec<Value> {
        let (status, body) = self.send(Method::GET, "/api/nutrition/meals", None).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        body["meals"].as_array().unwrap().clone()
    }

    async fn log(&self, body: Value) -> (StatusCode, Value) {
        self.send(Method::POST, "/api/nutrition/meal-logs", Some(body))
            .await
    }

    /// Logs `body`, expecting a new Meal Log, and returns it.
    async fn logged(&self, body: Value) -> Value {
        let (status, logged) = self.log(body).await;
        assert_eq!(status, StatusCode::CREATED, "{logged}");
        assert_eq!(logged["replayed"], false);
        logged["meal_log"].clone()
    }

    async fn day(&self, day: &str) -> Value {
        let (status, body) = self
            .send(Method::GET, &format!("/api/nutrition/days/{day}"), None)
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        body
    }

    async fn link(&self, barcode: &str, food_id: i64, request_id: &str) {
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

async fn open(dir: &TempDir, pin: DatasetPin) -> (FoodDatabase, Option<FoodImport>) {
    let url = format!("sqlite:{}", dir.path().join("food.db").display());
    let pool = db::connect_sqlite(&url).await.unwrap();
    FoodDatabase::open(pool, pin, dir.path().join("food-dataset"))
        .await
        .unwrap()
}

async fn import(server: &MockServer, dir: &TempDir, version: &str, bytes: Vec<u8>) -> FoodDatabase {
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

fn app(server: &MockServer, dir: &TempDir, foods: FoodDatabase, state: AppState) -> Router {
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

/// Skyr then chicken, as the user named them.
fn protein_bowl(request_id: &str) -> Value {
    json!({
        "name": "  Protein   bowl ",
        "portions": [{"food_id": 1693, "grams": 150}, {"food_id": 907, "grams": 120}],
        "request_id": request_id,
    })
}

/// Logs the Meal named `name` at breakfast time on 2026-10-29.
fn by_name(name: &str, request_id: &str) -> Value {
    json!({
        "eaten_at": "2026-10-29T08:15:00+01:00",
        "meal": name,
        "request_id": request_id,
    })
}

#[tokio::test]
async fn a_saved_meal_lists_its_ordered_items_by_reference_under_a_case_insensitively_unique_name()
{
    let api = Api::start().await;

    let meal = api.saved(protein_bowl("bowl-1")).await;

    assert_eq!(meal["name"], "Protein bowl");
    assert_eq!(
        meal["portions"],
        json!([
            {
                "food_id": 1693, "barcode": null, "grams": 150.0,
                "name_da": "Skyr, 0.2 % fedt", "name_en": "Skyr, 0.2 % fat", "product": null,
            },
            {
                "food_id": 907, "barcode": null, "grams": 120.0,
                "name_da": "Kylling, bryst, kød og skind, rå",
                "name_en": "Chicken, breast, flesh and skin, raw", "product": null,
            },
        ])
    );
    // A Meal holds references, never values.
    assert!(meal["portions"][0].get("nutrients").is_none());
    let banana = api
        .saved(json!({
            "name": "banana",
            "portions": [{"food_id": 3, "grams": 120}],
            "request_id": "banana-1",
        }))
        .await;
    assert_eq!(api.meals().await, [banana, meal]);

    let mut shouted = protein_bowl("bowl-2");
    shouted["name"] = json!("PROTEIN BOWL");
    let (status, body) = api.save(shouted).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(api.meals().await.len(), 2);
}

#[tokio::test]
async fn a_retried_save_replays_conflicts_on_changes_and_never_recreates_a_deleted_meal() {
    let api = Api::start().await;
    let meal = api.saved(protein_bowl("bowl-1")).await;

    let mut retry = protein_bowl("bowl-1");
    retry["name"] = json!("Protein bowl");
    let (status, replayed) = api.save(retry).await;
    assert_eq!(status, StatusCode::OK, "{replayed}");
    assert_eq!(replayed, json!({"meal": meal, "replayed": true}));

    let mut changed = protein_bowl("bowl-1");
    changed["portions"][0]["grams"] = json!(200);
    let (status, body) = api.save(changed).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(api.meals().await, std::slice::from_ref(&meal));

    assert_eq!(api.delete(&meal["id"]).await, StatusCode::NO_CONTENT);
    let (status, body) = api.save(protein_bowl("bowl-1")).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert!(
        body["error"].as_str().unwrap().contains("deleted"),
        "{body}"
    );
    assert_eq!(api.meals().await, Vec::<Value>::new());

    // A rejected request consumes no request_id.
    let mut unknown = protein_bowl("rejected-first");
    unknown["portions"][1]["food_id"] = json!(424_242);
    let (status, body) = api.save(unknown).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    api.saved(protein_bowl("rejected-first")).await;
}

#[tokio::test]
async fn renaming_a_meal_to_another_meals_name_conflicts_and_a_change_of_case_is_allowed() {
    let api = Api::start().await;
    let bowl = api.saved(protein_bowl("bowl-1")).await;
    let banana = api
        .saved(json!({
            "name": "Banana",
            "portions": [{"food_id": 3, "grams": 120}],
            "request_id": "banana-1",
        }))
        .await;

    let (status, body) = api
        .update(&banana["id"], json!({"name": "protein BOWL"}))
        .await;

    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(api.meals().await, [banana.clone(), bowl.clone()]);
    let (status, renamed) = api
        .update(&bowl["id"], json!({"name": "PROTEIN BOWL"}))
        .await;
    assert_eq!(status, StatusCode::OK, "{renamed}");
    assert_eq!(
        (&renamed["name"], &renamed["portions"]),
        (&json!("PROTEIN BOWL"), &bowl["portions"])
    );
}

#[tokio::test]
async fn logging_a_meal_by_name_copies_its_items_as_portions_with_overrides_and_records_its_name() {
    let api = Api::start().await;
    let meal = api.saved(protein_bowl("bowl-1")).await;
    let (status, body) = api
        .send(
            Method::PUT,
            "/api/nutrition/foods/1693/nova-class",
            Some(json!({"nova_class": 1})),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let mut request = by_name("protein BOWL", "log-1");
    request["overrides"] = json!([{"food_id": 1693, "grams": 200}]);
    request["note"] = json!("bigger skyr today");
    let meal_log = api.logged(request).await;

    assert_eq!(meal_log["meal_name"], "Protein bowl");
    assert_eq!(meal_log["note"], "bigger skyr today");
    let portions: Vec<(&Value, &Value, &Value, &Value, &Value)> = meal_log["portions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|portion| {
            (
                &portion["food_id"],
                &portion["grams"],
                &portion["nutrients"]["energy_kcal"],
                &portion["nutrients"]["protein"]["value"],
                &portion["nova_class"],
            )
        })
        .collect();
    assert_eq!(
        portions,
        [
            (
                &json!(1693),
                &json!(200.0),
                &json!({"value": 124.0, "unit": "kcal", "source": "food"}),
                &json!(22.0),
                &json!(1)
            ),
            (
                &json!(907),
                &json!(120.0),
                &json!({"value": 180.0, "unit": "kcal", "source": "food"}),
                &json!(25.2),
                &Value::Null
            ),
        ]
    );
    assert_eq!(
        meal_log["totals"]["energy_kcal"],
        json!({
            "value": 304.0, "unit": "kcal", "unknown_portions": 0,
            "food": 304.0, "supplements": null,
        })
    );
    let day = api.day("2026-10-29").await;
    assert_eq!(day["meal_logs"], json!([meal_log]));
    assert_eq!(day["nova"]["nova_1_kcal"], 124.0);
    // The override was one-off: the Meal keeps its grams.
    assert_eq!(api.meals().await, [meal]);

    // Without overrides every item keeps the Meal's grams.
    let mut lunch = by_name("Protein bowl", "log-2");
    lunch["eaten_at"] = json!("2026-10-29T12:00:00+01:00");
    let lunch = api.logged(lunch).await;
    assert_eq!(lunch["portions"][0]["grams"], 150.0);
    assert_eq!(lunch["portions"][1]["grams"], 120.0);
}

#[tokio::test]
async fn editing_or_deleting_a_meal_leaves_its_meal_logs_unchanged() {
    let api = Api::start().await;
    let meal = api.saved(protein_bowl("bowl-1")).await;
    let mut request = by_name("Protein bowl", "log-1");
    request["overrides"] = json!([{"food_id": 907, "grams": 100}]);
    let breakfast = api.logged(request.clone()).await;

    let (status, edited) = api
        .update(
            &meal["id"],
            json!({"name": "Chicken plate", "portions": [{"food_id": 907, "grams": 300}]}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{edited}");

    assert_eq!(api.day("2026-10-29").await["meal_logs"], json!([breakfast]));
    // A retried log replays what it logged, whatever became of the Meal.
    let (status, replayed) = api.log(request.clone()).await;
    assert_eq!(status, StatusCode::OK, "{replayed}");
    assert_eq!(replayed["meal_log"], breakfast);
    let (status, body) = api.log(by_name("Protein bowl", "log-2")).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let mut dinner = by_name("chicken plate", "log-3");
    dinner["eaten_at"] = json!("2026-10-29T19:00:00+01:00");
    let dinner = api.logged(dinner).await;
    assert_eq!(dinner["meal_name"], "Chicken plate");
    assert_eq!(dinner["portions"].as_array().unwrap().len(), 1);
    assert_eq!(dinner["portions"][0]["grams"], 300.0);

    assert_eq!(api.delete(&meal["id"]).await, StatusCode::NO_CONTENT);

    assert_eq!(
        api.day("2026-10-29").await["meal_logs"],
        json!([breakfast, dinner])
    );
    let (status, replayed) = api.log(request).await;
    assert_eq!(status, StatusCode::OK, "{replayed}");
    assert_eq!(api.meals().await, Vec::<Value>::new());
    for status in [
        api.update(&meal["id"], json!({"name": "Gone"})).await.0,
        api.delete(&meal["id"]).await,
    ] {
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
}

#[tokio::test]
async fn a_meal_references_a_linked_product_by_barcode_so_a_relink_reaches_its_next_log() {
    let api = Api::start().await;
    let unlinked = json!({
        "name": "Skyr snack",
        "portions": [{"barcode": UNLINKED, "grams": 100}],
        "request_id": "snack-0",
    });
    let (status, body) = api.save(unlinked).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert!(
        body["error"].as_str().unwrap().contains("link_product"),
        "{body}"
    );
    assert_eq!(api.meals().await, Vec::<Value>::new());
    api.link(SKYR, 1693, "link-1").await;

    let meal = api
        .saved(json!({
            "name": "Skyr snack",
            "portions": [{"barcode": SKYR, "grams": 100}, {"food_id": 3, "grams": 120}],
            "request_id": "snack-1",
        }))
        .await;

    assert_eq!(
        meal["portions"][0],
        json!({
            "food_id": null, "barcode": SKYR, "grams": 100.0,
            "name_da": "Skyr, 0.2 % fedt", "name_en": "Skyr, 0.2 % fat",
            "product": {"barcode": SKYR, "name": "Vanilla Skyr", "brand": "Synthetic Dairy"},
        })
    );
    let first = api.logged(by_name("skyr snack", "log-1")).await;
    let skyr = &first["portions"][0];
    assert_eq!(
        (
            &skyr["product"]["barcode"],
            &skyr["food_id"],
            &skyr["nova_class"],
            &skyr["nutrients"]["energy_kcal"],
            &skyr["nutrients"]["dietary_fibre"],
        ),
        (
            &json!(SKYR),
            &json!(1693),
            &json!(3),
            &json!({"value": 80.0, "unit": "kcal", "source": "label"}),
            &json!({"value": 0.0, "unit": "g", "source": "food"}),
        )
    );

    // Re-linked to the chicken, which has no fibre value: the next log fills its gaps from it.
    api.link(SKYR, 907, "link-2").await;
    let mut later = by_name("Skyr snack", "log-2");
    later["eaten_at"] = json!("2026-10-29T15:00:00+01:00");
    later["overrides"] = json!([{"barcode": SKYR, "grams": 50}]);
    let second = api.logged(later).await;

    let skyr = &second["portions"][0];
    assert_eq!(
        (
            &skyr["food_id"],
            &skyr["grams"],
            &skyr["nutrients"]["energy_kcal"]["value"],
            &skyr["nutrients"]["dietary_fibre"],
        ),
        (
            &json!(907),
            &json!(50.0),
            &json!(40.0),
            &json!({"value": null, "unit": "g", "source": null}),
        )
    );
    assert_eq!(
        api.day("2026-10-29").await["meal_logs"],
        json!([first, second])
    );
    assert_eq!(
        api.meals().await[0]["portions"][0]["name_en"],
        "Chicken, breast, flesh and skin, raw"
    );
}

#[tokio::test]
async fn a_meal_whose_food_left_the_food_database_must_be_updated_before_it_is_logged() {
    let mut api = Api::start().await;
    let meal = api
        .saved(json!({
            "name": "Banana skyr",
            "portions": [{"food_id": 3, "grams": 120}, {"food_id": 1693, "grams": 150}],
            "request_id": "banana-skyr-1",
        }))
        .await;
    // Version 2 drops the Banana and changes Skyr's protein.
    api.reimport("2", second_dataset()).await;

    let (status, body) = api.log(by_name("Banana skyr", "log-1")).await;

    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["code"], "meal_needs_update");
    assert!(
        body["error"]
            .as_str()
            .unwrap()
            .contains("Food 3 \"Banana, raw\""),
        "{body}"
    );
    assert_eq!(api.day("2026-10-29").await["meal_logs"], json!([]));
    // The Meal still lists the Food by the names it was saved with.
    assert_eq!(api.meals().await, std::slice::from_ref(&meal));
    let (status, updated) = api
        .update(
            &meal["id"],
            json!({"portions": [{"food_id": 2, "grams": 150}, {"food_id": 1693, "grams": 150}]}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    let meal_log = api.logged(by_name("Banana skyr", "log-1")).await;
    assert_eq!(
        meal_log["portions"][0]["name_en"],
        "Apple, raw, all varieties"
    );
    assert_eq!(
        meal_log["portions"][1]["nutrients"]["protein"]["value"],
        18.0
    );
    assert_eq!(meal_log["portions"][1]["dataset_version"], "2");

    // Version 3 gives FoodID 2 to another Food: the Meal must not log it as an apple.
    api.reimport(
        "3",
        common::food_workbook::workbook(
            &common::food_workbook::parameters("g/100g"),
            &[
                common::food_workbook::SyntheticFood {
                    id: 2,
                    ..common::food_workbook::rye_bread()
                },
                common::food_workbook::skyr(12.0),
            ],
        ),
    )
    .await;
    let mut renumbered = by_name("Banana skyr", "log-2");
    renumbered["eaten_at"] = json!("2026-10-29T12:00:00+01:00");
    let (status, body) = api.log(renumbered).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert!(
        body["error"]
            .as_str()
            .unwrap()
            .contains("Food 2 \"Apple, raw, all varieties\""),
        "{body}"
    );
}

#[tokio::test]
async fn invalid_meals_and_logs_by_name_are_refused_without_writing() {
    let api = Api::start().await;
    let long_name = "x".repeat(101);
    for (body, expected) in [
        (json!({"name": "   "}), StatusCode::BAD_REQUEST),
        (json!({"name": long_name}), StatusCode::BAD_REQUEST),
        (json!({"name": "Tab\tbed\u{7}"}), StatusCode::BAD_REQUEST),
        (json!({"portions": []}), StatusCode::BAD_REQUEST),
        (
            json!({"portions": [{"food_id": 1693, "barcode": SKYR, "grams": 10}]}),
            StatusCode::BAD_REQUEST,
        ),
        (
            json!({"portions": [{"food_id": 1693, "grams": 100}, {"food_id": 1693, "grams": 50}]}),
            StatusCode::BAD_REQUEST,
        ),
        (
            json!({"portions": [{"food_id": 1693, "grams": 0}]}),
            StatusCode::BAD_REQUEST,
        ),
        // A Meal holds references, never values.
        (
            json!({"portions": [{"food_id": 1693, "grams": 10, "estimates": {"vitamin_d": 1}}]}),
            StatusCode::BAD_REQUEST,
        ),
        (json!({"request_id": " "}), StatusCode::BAD_REQUEST),
        (
            json!({"portions": [{"food_id": 424_242, "grams": 10}]}),
            StatusCode::NOT_FOUND,
        ),
    ] {
        let mut request = protein_bowl("bowl-1");
        for (key, value) in body.as_object().unwrap() {
            request[key] = value.clone();
        }
        let (status, response) = api.save(request.clone()).await;
        assert_eq!(status, expected, "{request}: {response}");
    }
    assert_eq!(api.meals().await, Vec::<Value>::new());

    let meal = api.saved(protein_bowl("bowl-1")).await;
    for edit in [
        json!({"name": null}),
        json!({"portions": null}),
        json!({"name": ""}),
    ] {
        let (status, body) = api.update(&meal["id"], edit.clone()).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{edit}: {body}");
    }
    assert_eq!(api.meals().await, [meal]);

    let both = json!({"portions": [{"food_id": 1693, "grams": 10}]});
    for (change, expected) in [
        (both, StatusCode::BAD_REQUEST),
        (json!({"meal": null}), StatusCode::BAD_REQUEST),
        (json!({"meal": "Lasagne"}), StatusCode::NOT_FOUND),
        (
            json!({"overrides": [{"food_id": 3, "grams": 10}]}),
            StatusCode::BAD_REQUEST,
        ),
        (
            json!({"overrides": [{"food_id": 1693, "grams": 10}, {"food_id": 1693, "grams": 20}]}),
            StatusCode::BAD_REQUEST,
        ),
        (
            json!({"overrides": [{"food_id": 1693, "grams": 0}]}),
            StatusCode::BAD_REQUEST,
        ),
        (
            json!({"overrides": [{"food_id": 1693, "grams": 10, "estimates": {}}]}),
            StatusCode::BAD_REQUEST,
        ),
    ] {
        let mut request = by_name("Protein bowl", "log-1");
        for (key, value) in change.as_object().unwrap() {
            request[key] = value.clone();
        }
        let (status, body) = api.log(request.clone()).await;
        assert_eq!(status, expected, "{request}: {body}");
    }
    let overrides_without_meal = json!({
        "eaten_at": "2026-10-29T08:15:00+01:00",
        "portions": [{"food_id": 1693, "grams": 10}],
        "overrides": [{"food_id": 1693, "grams": 20}],
        "request_id": "log-1",
    });
    let (status, body) = api.log(overrides_without_meal).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(api.day("2026-10-29").await["meal_logs"], json!([]));
    // None of the rejected logs consumed the request_id.
    api.logged(by_name("Protein bowl", "log-1")).await;
}

#[tokio::test]
async fn while_the_food_database_is_unavailable_meals_stay_listable_renamable_and_deletable() {
    let api = Api::start().await;
    let meal = api.saved(protein_bowl("bowl-1")).await;
    let api = api.importing().await;

    for (status, body) in [
        api.save(json!({
            "name": "Chicken",
            "portions": [{"food_id": 907, "grams": 100}],
            "request_id": "chicken-1",
        }))
        .await,
        api.update(
            &meal["id"],
            json!({"portions": [{"food_id": 907, "grams": 100}]}),
        )
        .await,
        api.log(by_name("Protein bowl", "log-1")).await,
    ] {
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
        assert_eq!(
            (&body["code"], &body["state"]),
            (&json!("food_database_unavailable"), &json!("importing"))
        );
    }
    assert_eq!(api.meals().await, std::slice::from_ref(&meal));
    let (status, renamed) = api.update(&meal["id"], json!({"name": "Bowl"})).await;
    assert_eq!(status, StatusCode::OK, "{renamed}");
    assert_eq!(renamed["name"], "Bowl");
    assert_eq!(api.delete(&meal["id"]).await, StatusCode::NO_CONTENT);
}
