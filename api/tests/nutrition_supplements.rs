//! Supplements and Supplement Doses through the nutrition routes, over a real `app.db` and a real
//! `food.db` imported from a synthetic DTU-shaped workbook served by `wiremock`.

mod common;

use axum::{Router, http::Method, http::StatusCode};
use common::food_workbook::{first_dataset, sha256};
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

/// The nutrition routes over one `app.db` and a `food.db` imported from `wiremock`.
struct Api {
    server: MockServer,
    dir: TempDir,
    state: AppState,
    app: Router,
}

impl Api {
    async fn start() -> Self {
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
        let app = app(&self.dir, foods, self.state.clone());
        Self { app, ..self }
    }

    async fn send(&self, method: Method, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
        send_request_with_method(self.app.clone(), uri, method, body, Some(API_KEY)).await
    }

    async fn add(&self, body: Value) -> (StatusCode, Value) {
        self.send(Method::POST, "/api/nutrition/supplements", Some(body))
            .await
    }

    /// Adds `body`, expecting a new Supplement, and returns it.
    async fn added(&self, body: Value) -> Value {
        let (status, added) = self.add(body).await;
        assert_eq!(status, StatusCode::CREATED, "{added}");
        assert_eq!(added["replayed"], false);
        added["supplement"].clone()
    }

    async fn update(&self, id: &Value, body: Value) -> (StatusCode, Value) {
        self.send(
            Method::PATCH,
            &format!("/api/nutrition/supplements/{}", id.as_str().unwrap()),
            Some(body),
        )
        .await
    }

    async fn delete(&self, id: &Value) -> (StatusCode, Value) {
        self.send(
            Method::DELETE,
            &format!("/api/nutrition/supplements/{}", id.as_str().unwrap()),
            None,
        )
        .await
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

    async fn update_log(&self, id: &Value, body: Value) -> (StatusCode, Value) {
        self.send(
            Method::PATCH,
            &format!("/api/nutrition/meal-logs/{}", id.as_str().unwrap()),
            Some(body),
        )
        .await
    }

    async fn meal_logs(&self, day: &str) -> Vec<Value> {
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

    async fn day(&self, day: &str) -> Value {
        let (status, body) = self
            .send(Method::GET, &format!("/api/nutrition/days/{day}"), None)
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        body
    }

    /// The day's total of the nutrient `key`.
    async fn nutrient(&self, day: &str, key: &str) -> Value {
        let day = self.day(day).await;
        day["nutrients"]
            .as_array()
            .unwrap()
            .iter()
            .find(|nutrient| nutrient["key"] == key)
            .cloned()
            .unwrap_or_else(|| panic!("{key} is missing from {day}"))
    }

    async fn save_meal(&self, body: Value) -> (StatusCode, Value) {
        self.send(Method::POST, "/api/nutrition/meals", Some(body))
            .await
    }

    async fn update_meal(&self, id: &Value, body: Value) -> (StatusCode, Value) {
        self.send(
            Method::PATCH,
            &format!("/api/nutrition/meals/{}", id.as_str().unwrap()),
            Some(body),
        )
        .await
    }

    async fn delete_meal(&self, id: &Value) -> StatusCode {
        self.send(
            Method::DELETE,
            &format!("/api/nutrition/meals/{}", id.as_str().unwrap()),
            None,
        )
        .await
        .0
    }

    async fn supplements(&self) -> Vec<Value> {
        let (status, body) = self
            .send(Method::GET, "/api/nutrition/supplements", None)
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        body["supplements"].as_array().unwrap().clone()
    }
}

async fn open(dir: &TempDir, pin: DatasetPin) -> (FoodDatabase, Option<FoodImport>) {
    let url = format!("sqlite:{}", dir.path().join("food.db").display());
    let pool = db::connect_sqlite(&url).await.unwrap();
    FoodDatabase::open(pool, pin, dir.path().join("food-dataset"))
        .await
        .unwrap()
}

fn app(dir: &TempDir, foods: FoodDatabase, state: AppState) -> Router {
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
fn fish_oil(request_id: &str) -> Value {
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
fn creatine(request_id: &str) -> Value {
    json!({
        "name": "Creatine",
        "dose_unit": "scoop",
        "compounds": [{"name": "Creatine  monohydrate", "value": 3, "unit": "g"}],
        "request_id": request_id,
    })
}

#[tokio::test]
async fn a_supplement_keeps_per_dose_amounts_in_parameter_units_under_a_unique_name() {
    let api = Api::start().await;

    let oil = api.added(fish_oil("oil-1")).await;

    assert_eq!(oil["name"], "Omega-3 fish oil");
    assert_eq!(oil["dose_unit"], "capsule");
    // Label units become the parameter's unit, and energy is completed in kJ as for labels.
    assert_eq!(
        oil["amounts"],
        json!([
            {"key": "epa", "form": null, "value": 0.18, "unit": "g"},
            {"key": "dha", "form": null, "value": 0.12, "unit": "g"},
            {"key": "vitamin_d", "form": null, "value": 5.0, "unit": "µg"},
            {"key": "energy_kcal", "form": null, "value": 9.0, "unit": "kcal"},
            {"key": "fat", "form": null, "value": 1.0, "unit": "g"},
            {"key": "energy_kj", "form": null, "value": 37.656, "unit": "kJ"},
        ])
    );
    assert_eq!(oil["compounds"], json!([]));
    let creatine = api.added(creatine("creatine-1")).await;
    assert_eq!(
        creatine["compounds"],
        json!([{"name": "Creatine monohydrate", "value": 3.0, "unit": "g"}])
    );
    assert_eq!(api.supplements().await, [creatine, oil.clone()]);

    let mut shouted = fish_oil("oil-2");
    shouted["name"] = json!("OMEGA-3 FISH OIL");
    let (status, body) = api.add(shouted).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(api.supplements().await.len(), 2);
}

#[tokio::test]
async fn supplement_only_upper_limit_forms_are_named_on_their_amounts() {
    let api = Api::start().await;

    let multivitamin = api
        .added(json!({
            "name": "Multivitamin",
            "dose_unit": "tablet",
            "amounts": [
                {"key": "vitamin_a", "value": 400, "unit": "µg", "form": "retinol"},
                {"key": "vitamin_a", "value": 400, "unit": "µg RE", "form": "beta_carotene"},
                {"key": "folate", "value": 0.2, "unit": "mg", "form": "folic_acid"},
                {"key": "niacin_equivalent", "value": 16, "unit": "mg", "form": "nicotinamide"},
                {"key": "magnesium", "value": 100, "unit": "mg"},
            ],
            "request_id": "multi-1",
        }))
        .await;

    assert_eq!(
        multivitamin["amounts"],
        json!([
            {"key": "vitamin_a", "form": "retinol", "value": 400.0, "unit": "µg RE"},
            {"key": "vitamin_a", "form": "beta_carotene", "value": 400.0, "unit": "µg RE"},
            {"key": "folate", "form": "folic_acid", "value": 200.0, "unit": "µg"},
            {"key": "niacin_equivalent", "form": "nicotinamide", "value": 16.0, "unit": "mg NE"},
            {"key": "magnesium", "form": null, "value": 100.0, "unit": "mg"},
        ])
    );
}

#[tokio::test]
async fn a_supplement_amount_needs_a_parameter_key_its_unit_and_a_form_where_limits_differ() {
    let api = Api::start().await;
    let refused = [
        json!({"key": "vitamin_q", "value": 1, "unit": "mg"}),
        json!({"key": "waste", "value": 1, "unit": "%"}),
        json!({"key": "vitamin_d", "value": 1, "unit": "kcal"}),
        json!({"key": "vitamin_d", "value": -1, "unit": "µg"}),
        json!({"key": "folate", "value": 400, "unit": "µg"}),
        json!({"key": "folate", "value": 400, "unit": "µg", "form": "retinol"}),
        json!({"key": "zinc", "value": 10, "unit": "mg", "form": "folic_acid"}),
    ];

    for amount in refused {
        let (status, body) = api
            .add(json!({
                "name": "Refused",
                "dose_unit": "tablet",
                "amounts": [amount],
                "request_id": "refused",
            }))
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{amount}: {body}");
    }
    let (_, body) = api
        .add(json!({
            "name": "Folate",
            "dose_unit": "tablet",
            "amounts": [{"key": "folate", "value": 400, "unit": "µg"}],
            "request_id": "folate",
        }))
        .await;
    assert!(
        body["error"].as_str().unwrap().contains("folic_acid"),
        "{body}"
    );
    for supplement in [
        json!({"name": "Twice", "dose_unit": "tablet", "request_id": "twice", "amounts": [
            {"key": "zinc", "value": 10, "unit": "mg"}, {"key": "zinc", "value": 5, "unit": "mg"},
        ]}),
        json!({"name": "Empty", "dose_unit": "tablet", "request_id": "empty"}),
        json!({"name": "Pill", "dose_unit": "pill", "request_id": "pill", "amounts": [
            {"key": "zinc", "value": 10, "unit": "mg"},
        ]}),
        // A parameter is never a named compound.
        json!({"name": "D", "dose_unit": "tablet", "request_id": "d", "compounds": [
            {"name": "Vitamin D", "value": 25, "unit": "µg"},
        ]}),
        json!({"name": "Probiotic", "dose_unit": "capsule", "request_id": "cfu", "compounds": [
            {"name": "Lactobacillus", "value": 10, "unit": "billion"},
        ]}),
    ] {
        let (status, body) = api.add(supplement.clone()).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{supplement}: {body}");
    }
    assert_eq!(api.supplements().await, Vec::<Value>::new());
}

#[tokio::test]
async fn a_retried_addition_replays_conflicts_on_changes_and_never_recreates_a_deleted_supplement()
{
    let api = Api::start().await;
    let oil = api.added(fish_oil("oil-1")).await;

    let (status, replayed) = api.add(fish_oil("oil-1")).await;
    assert_eq!(status, StatusCode::OK, "{replayed}");
    assert_eq!(replayed, json!({"supplement": oil, "replayed": true}));

    let mut changed = fish_oil("oil-1");
    changed["amounts"][0]["value"] = json!(200);
    let (status, body) = api.add(changed).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");

    let (status, _) = api.delete(&oil["id"]).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, body) = api.add(fish_oil("oil-1")).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert!(
        body["error"].as_str().unwrap().contains("deleted"),
        "{body}"
    );
    assert_eq!(api.supplements().await, Vec::<Value>::new());
}

#[tokio::test]
async fn updating_a_supplement_renames_it_or_replaces_its_amounts_together() {
    let api = Api::start().await;
    let oil = api.added(fish_oil("oil-1")).await;
    let creatine = api.added(creatine("creatine-1")).await;

    let (status, updated) = api
        .update(
            &oil["id"],
            json!({
                "name": "Fish oil",
                "dose_unit": "ml",
                "amounts": [{"key": "epa", "value": 0.3, "unit": "g"}],
            }),
        )
        .await;

    assert_eq!(status, StatusCode::OK, "{updated}");
    assert_eq!(
        updated,
        json!({
            "id": oil["id"], "name": "Fish oil", "dose_unit": "ml",
            "amounts": [{"key": "epa", "form": null, "value": 0.3, "unit": "g"}],
            "compounds": [],
        })
    );
    let (status, body) = api.update(&oil["id"], json!({"name": "creatine"})).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    // A Supplement keeps at least one amount or compound.
    let (status, body) = api.update(&creatine["id"], json!({"compounds": []})).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(api.supplements().await, [creatine, updated]);
    let (status, body) = api
        .update(&json!("missing"), json!({"name": "Ghost"}))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
}

/// Breakfast on 2026-10-29: chicken with the doses given.
fn breakfast(request_id: &str, doses: &Value) -> Value {
    json!({
        "eaten_at": "2026-10-29T08:15:00+01:00",
        "portions": [{"food_id": 907, "grams": 200}],
        "supplement_doses": doses,
        "request_id": request_id,
    })
}

#[tokio::test]
async fn a_supplement_dose_counts_its_per_dose_amounts_times_the_doses_taken() {
    let api = Api::start().await;
    let oil = api.added(fish_oil("oil")).await;
    let creatine = api.added(creatine("creatine")).await;

    let log = api
        .logged(breakfast(
            "breakfast",
            &json!([
                {"supplement_id": oil["id"], "doses": 2},
                {"supplement_id": creatine["id"], "doses": 1},
            ]),
        ))
        .await;

    assert_eq!(
        log["supplement_doses"],
        json!([
            {
                "supplement_id": oil["id"], "name": "Omega-3 fish oil", "dose_unit": "capsule",
                "doses": 2.0,
                "amounts": [
                    {"key": "epa", "form": null, "value": 0.36, "unit": "g"},
                    {"key": "dha", "form": null, "value": 0.24, "unit": "g"},
                    {"key": "vitamin_d", "form": null, "value": 10.0, "unit": "µg"},
                    {"key": "energy_kcal", "form": null, "value": 18.0, "unit": "kcal"},
                    {"key": "fat", "form": null, "value": 2.0, "unit": "g"},
                    {"key": "energy_kj", "form": null, "value": 75.312, "unit": "kJ"},
                ],
                "compounds": [],
            },
            {
                "supplement_id": creatine["id"], "name": "Creatine", "dose_unit": "scoop",
                "doses": 1.0, "amounts": [],
                "compounds": [{"name": "Creatine monohydrate", "value": 3.0, "unit": "g"}],
            },
        ])
    );
    // 200 g chicken has 300 kcal and 14 g fat; the label's energy and fat count toward eaten.
    assert_eq!(
        (
            &log["totals"]["energy_kcal"],
            &log["totals"]["fat"],
            &log["totals"]["protein"]
        ),
        (
            &json!({
                "value": 318.0, "unit": "kcal", "unknown_portions": 0,
                "food": 300.0, "supplements": 18.0,
            }),
            &json!({
                "value": 16.0, "unit": "g", "unknown_portions": 0,
                "food": 14.0, "supplements": 2.0,
            }),
            &json!({
                "value": 42.0, "unit": "g", "unknown_portions": 0,
                "food": 42.0, "supplements": null,
            }),
        )
    );
}

#[tokio::test]
async fn the_day_splits_every_total_into_food_and_supplement_shares_and_lists_compounds_taken() {
    let api = Api::start().await;
    let oil = api.added(fish_oil("oil")).await;
    let creatine = api.added(creatine("creatine")).await;
    api.logged(breakfast(
        "breakfast",
        &json!([
            {"supplement_id": oil["id"], "doses": 2},
            {"supplement_id": creatine["id"], "doses": 1},
        ]),
    ))
    .await;
    // An evening stack without food.
    api.logged(json!({
        "eaten_at": "2026-10-29T21:00:00+01:00",
        "supplement_doses": [
            {"supplement_id": oil["id"], "doses": 1},
            {"supplement_id": creatine["id"], "doses": 1},
        ],
        "request_id": "evening",
    }))
    .await;

    let day = api.day("2026-10-29").await;

    // Chicken has 0.01 g EPA per 100 g; three capsules add 0.54 g.
    assert_eq!(
        api.nutrient("2026-10-29", "epa").await,
        json!({
            "key": "epa", "name_da": "C20:5,n-3", "name_en": "C20:5,n-3",
            "group": {"name_da": "Flerumættede fedtsyrer", "name_en": "Polyunsaturated fatty acids"},
            "unit": "g", "value": 0.56, "unknown_portions": 0, "food": 0.02, "supplements": 0.54,
        })
    );
    // Chicken has no vitamin D value: the supplement share is all that is known.
    let vitamin_d = api.nutrient("2026-10-29", "vitamin_d").await;
    assert_eq!(
        (
            &vitamin_d["value"],
            &vitamin_d["food"],
            &vitamin_d["supplements"]
        ),
        (&json!(15.0), &Value::Null, &json!(15.0))
    );
    assert_eq!(vitamin_d["unknown_portions"], 1);
    // DHA is not in this Food database's parameters, but a Supplement gives it by key.
    let dha = api.nutrient("2026-10-29", "dha").await;
    assert_eq!(
        (
            &dha["name_en"],
            &dha["unit"],
            &dha["value"],
            &dha["supplements"]
        ),
        (&Value::Null, &json!("g"), &json!(0.36), &json!(0.36))
    );
    assert_eq!(
        day["totals"]["energy_kcal"],
        json!({
            "value": 327.0, "unit": "kcal", "unknown_portions": 0,
            "food": 300.0, "supplements": 27.0,
        })
    );
    assert_eq!(
        day["compounds"],
        json!([{"name": "Creatine monohydrate", "value": 6.0, "unit": "g"}])
    );
    assert_eq!(
        day["supplement_doses"],
        json!([
            {
                "supplement_id": oil["id"], "name": "Omega-3 fish oil",
                "dose_unit": "capsule", "doses": 3.0,
            },
            {
                "supplement_id": creatine["id"], "name": "Creatine",
                "dose_unit": "scoop", "doses": 2.0,
            },
        ])
    );
    // A Supplement's energy is not a classified Food, so the NOVA share leaves it out.
    assert_eq!(day["nova"]["unclassified_portions"], 1);
}

#[tokio::test]
async fn the_day_gives_the_supplement_share_of_each_upper_limit_form() {
    let api = Api::start().await;
    let multivitamin = api
        .added(json!({
            "name": "Multivitamin",
            "dose_unit": "tablet",
            "amounts": [
                {"key": "vitamin_a", "value": 300, "unit": "µg", "form": "retinol"},
                {"key": "vitamin_a", "value": 100, "unit": "µg", "form": "beta_carotene"},
                {"key": "folate", "value": 200, "unit": "µg", "form": "folic_acid"},
                {"key": "magnesium", "value": 100, "unit": "mg"},
            ],
            "request_id": "multi",
        }))
        .await;

    api.logged(breakfast(
        "breakfast",
        &json!([{"supplement_id": multivitamin["id"], "doses": 2}]),
    ))
    .await;

    let vitamin_a = api.nutrient("2026-10-29", "vitamin_a").await;
    assert_eq!(
        (&vitamin_a["supplements"], &vitamin_a["supplement_forms"]),
        (
            &json!(800.0),
            &json!([
                {"form": "beta_carotene", "value": 200.0},
                {"form": "retinol", "value": 600.0},
            ])
        )
    );
    let folate = api.nutrient("2026-10-29", "folate").await;
    assert_eq!(
        folate["supplement_forms"],
        json!([{"form": "folic_acid", "value": 400.0}])
    );
    // Magnesium's supplement-only limit is simply its supplement share.
    let magnesium = api.nutrient("2026-10-29", "magnesium").await;
    assert_eq!(
        (&magnesium["supplements"], magnesium.get("supplement_forms")),
        (&json!(200.0), None)
    );
}

#[tokio::test]
async fn a_logged_dose_keeps_its_snapshot_after_the_supplement_is_edited_or_deleted() {
    let api = Api::start().await;
    let oil = api.added(fish_oil("oil")).await;
    let creatine = api.added(creatine("creatine")).await;
    let log = api
        .logged(breakfast(
            "breakfast",
            &json!([
                {"supplement_id": oil["id"], "doses": 2},
                {"supplement_id": creatine["id"], "doses": 1},
            ]),
        ))
        .await;
    let before = api.day("2026-10-29").await;

    let (status, body) = api
        .update(
            &oil["id"],
            json!({
                "name": "Stronger oil",
                "amounts": [{"key": "epa", "value": 500, "unit": "mg"}],
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body) = api.delete(&creatine["id"]).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");

    assert_eq!(
        api.meal_logs("2026-10-29").await,
        std::slice::from_ref(&log)
    );
    assert_eq!(api.day("2026-10-29").await, before);
    // The edited label reaches the next dose.
    let next = api
        .logged(json!({
            "eaten_at": "2026-10-29T12:00:00+01:00",
            "supplement_doses": [{"supplement_id": oil["id"], "doses": 1}],
            "request_id": "lunch",
        }))
        .await;
    assert_eq!(
        (
            &next["supplement_doses"][0]["name"],
            &next["supplement_doses"][0]["amounts"]
        ),
        (
            &json!("Stronger oil"),
            &json!([{"key": "epa", "form": null, "value": 0.5, "unit": "g"}])
        )
    );
}

#[tokio::test]
async fn supplement_doses_are_checked_retried_and_replaced_like_portions() {
    let api = Api::start().await;
    let oil = api.added(fish_oil("oil")).await;
    let creatine = api.added(creatine("creatine")).await;
    let doses = json!([{"supplement_id": oil["id"], "doses": 2}]);
    let log = api.logged(breakfast("breakfast", &doses)).await;

    let (status, replayed) = api.log(breakfast("breakfast", &doses)).await;
    assert_eq!(status, StatusCode::OK, "{replayed}");
    assert_eq!(replayed["meal_log"], log);
    let (status, body) = api
        .log(breakfast(
            "breakfast",
            &json!([{"supplement_id": oil["id"], "doses": 3}]),
        ))
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");

    for (doses, expected) in [
        (
            json!([{"supplement_id": "missing", "doses": 1}]),
            StatusCode::NOT_FOUND,
        ),
        (
            json!([{"supplement_id": oil["id"], "doses": 0}]),
            StatusCode::BAD_REQUEST,
        ),
        (
            json!([{"supplement_id": oil["id"], "doses": 101}]),
            StatusCode::BAD_REQUEST,
        ),
        (
            json!([
                {"supplement_id": oil["id"], "doses": 1},
                {"supplement_id": oil["id"], "doses": 1},
            ]),
            StatusCode::BAD_REQUEST,
        ),
    ] {
        let (status, body) = api.log(breakfast("rejected", &doses)).await;
        assert_eq!(status, expected, "{doses}: {body}");
    }
    let (status, body) = api
        .log(json!({
            "eaten_at": "2026-10-29T08:15:00+01:00",
            "portions": [],
            "supplement_doses": [],
            "request_id": "nothing",
        }))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");

    let (status, updated) = api
        .update_log(
            &log["id"],
            json!({"supplement_doses": [{"supplement_id": creatine["id"], "doses": 2}]}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    assert_eq!(updated["portions"], log["portions"]);
    assert_eq!(
        updated["supplement_doses"][0]["compounds"],
        json!([{"name": "Creatine monohydrate", "value": 6.0, "unit": "g"}])
    );
    // Removing every Portion leaves the doses; removing both leaves nothing to log.
    let (status, updated) = api.update_log(&log["id"], json!({"portions": []})).await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    assert_eq!(updated["portions"], json!([]));
    let (status, body) = api
        .update_log(&log["id"], json!({"supplement_doses": []}))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
}

#[tokio::test]
async fn supplement_doses_are_logged_and_listed_while_the_food_database_imports() {
    let api = Api::start().await.importing().await;
    let oil = api.added(fish_oil("oil")).await;

    let log = api
        .logged(json!({
            "eaten_at": "2026-10-29T08:15:00+01:00",
            "supplement_doses": [{"supplement_id": oil["id"], "doses": 1}],
            "request_id": "stack",
        }))
        .await;

    assert_eq!(api.meal_logs("2026-10-29").await, [log]);
}

#[tokio::test]
async fn a_meal_takes_supplement_doses_by_reference_and_logs_them_with_dose_overrides() {
    let api = Api::start().await;
    let oil = api.added(fish_oil("oil")).await;
    let creatine = api.added(creatine("creatine")).await;
    let (status, saved) = api
        .save_meal(json!({
            "name": "Morning stack",
            "portions": [{"food_id": 1693, "grams": 150}],
            "supplement_doses": [
                {"supplement_id": oil["id"], "doses": 2},
                {"supplement_id": creatine["id"], "doses": 1},
            ],
            "request_id": "stack",
        }))
        .await;
    assert_eq!(status, StatusCode::CREATED, "{saved}");
    let meal = &saved["meal"];
    assert_eq!(
        meal["supplement_doses"],
        json!([
            {
                "supplement_id": oil["id"], "name": "Omega-3 fish oil",
                "dose_unit": "capsule", "doses": 2.0,
            },
            {
                "supplement_id": creatine["id"], "name": "Creatine",
                "dose_unit": "scoop", "doses": 1.0,
            },
        ])
    );
    // A Meal holds references, so a Supplement edit reaches its next log.
    api.update(
        &oil["id"],
        json!({"amounts": [{"key": "epa", "value": 250, "unit": "mg"}]}),
    )
    .await;

    let log = api
        .logged(json!({
            "eaten_at": "2026-10-29T07:30:00+01:00",
            "meal": "morning STACK",
            "overrides": [{"supplement_id": oil["id"], "doses": 1}],
            "request_id": "morning",
        }))
        .await;

    assert_eq!(log["meal_name"], "Morning stack");
    assert_eq!(log["portions"][0]["grams"], 150.0);
    assert_eq!(
        log["supplement_doses"]
            .as_array()
            .unwrap()
            .iter()
            .map(|dose| (dose["name"].clone(), dose["doses"].clone()))
            .collect::<Vec<_>>(),
        [
            (json!("Omega-3 fish oil"), json!(1.0)),
            (json!("Creatine"), json!(1.0)),
        ]
    );
    assert_eq!(
        log["supplement_doses"][0]["amounts"],
        json!([{"key": "epa", "form": null, "value": 0.25, "unit": "g"}])
    );
    let (status, body) = api
        .log(json!({
            "eaten_at": "2026-10-29T07:30:00+01:00",
            "meal": "Morning stack",
            "overrides": [{"supplement_id": "missing", "doses": 1}],
            "request_id": "bad-override",
        }))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");

    // A Supplement a Meal takes cannot be deleted until the Meal no longer takes it.
    let (status, body) = api.delete(&creatine["id"]).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert!(
        body["error"].as_str().unwrap().contains("Morning stack"),
        "{body}"
    );
    let (status, updated) = api
        .update_meal(
            &meal["id"],
            json!({"supplement_doses": [{"supplement_id": oil["id"], "doses": 2}]}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    assert_eq!(updated["portions"], meal["portions"]);
    assert_eq!(updated["supplement_doses"].as_array().unwrap().len(), 1);
    let (status, body) = api.delete(&creatine["id"]).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    assert_eq!(api.delete_meal(&meal["id"]).await, StatusCode::NO_CONTENT);
    let (status, body) = api.delete(&oil["id"]).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
}

#[tokio::test]
async fn a_meal_of_only_supplement_doses_is_saved_and_needs_at_least_one_item() {
    let api = Api::start().await;
    let oil = api.added(fish_oil("oil")).await;

    let (status, saved) = api
        .save_meal(json!({
            "name": "Evening stack",
            "supplement_doses": [{"supplement_id": oil["id"], "doses": 1}],
            "request_id": "evening",
        }))
        .await;

    assert_eq!(status, StatusCode::CREATED, "{saved}");
    assert_eq!(saved["meal"]["portions"], json!([]));
    let (status, body) = api
        .update_meal(&saved["meal"]["id"], json!({"supplement_doses": []}))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    for (doses, expected) in [
        (
            json!([{"supplement_id": "missing", "doses": 1}]),
            StatusCode::NOT_FOUND,
        ),
        (json!([]), StatusCode::BAD_REQUEST),
    ] {
        let (status, body) = api
            .save_meal(json!({
                "name": "Rejected",
                "supplement_doses": doses,
                "request_id": "rejected",
            }))
            .await;
        assert_eq!(status, expected, "{body}");
    }
}
