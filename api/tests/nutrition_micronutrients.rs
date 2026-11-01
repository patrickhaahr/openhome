//! The micronutrient panel through the Nutrition Day, and micronutrient coverage through the
//! Nutrition Summary: NNR 2023 Recommended Intakes by the Profile's sex and age band, and EFSA
//! Upper Limits checked against the day and against the trailing 7-day average, from a real `app.db` seeded through the Profile, Supplement and Meal
//! Log routes, and a real `food.db` imported from a synthetic DTU-shaped workbook served by
//! `wiremock`. Every Food and value here is invented.

mod common;

use std::sync::atomic::{AtomicUsize, Ordering};

use axum::{Router, http::Method, http::StatusCode};
use common::food_workbook::{MACRO, SyntheticFood, parameter, sha256, workbook};
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

const VITAMINS: (&str, &str) = ("Vitaminer", "Vitamins");
const MINERALS: (&str, &str) = (
    "Mineraler og uorganiske stoffer",
    "Minerals and inorganic substances",
);
const PUFA: (&str, &str) = ("Flerumættede fedtsyrer", "Polyunsaturated fatty acids");

/// Per 100 g: Orange 50 mg vitamin C and 30 µg folate; Salmon 10 µg vitamin D, 0.5 g EPA and
/// 1.2 g DHA; Pumpkin seeds 600 mg magnesium and 10 mg NE niacin; Liver pâté 4,000 µg RE
/// vitamin A, all of it retinol; Wholemeal bread 0.3 mg thiamin and 300 µg folate.
fn dataset() -> Vec<u8> {
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

/// The nutrition routes and the Profile route over one `app.db`.
struct Api {
    _server: MockServer,
    _dir: TempDir,
    nutrition: Router,
    fitness: Router,
    requests: AtomicUsize,
}

impl Api {
    async fn start() -> Self {
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

    async fn send(&self, method: Method, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
        send_request_with_method(self.nutrition.clone(), uri, method, body, Some(API_KEY)).await
    }

    fn request_id(&self) -> String {
        format!("request-{}", self.requests.fetch_add(1, Ordering::Relaxed))
    }

    async fn profile(&self, body: Value) {
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

    /// Registers a Supplement taking one capsule per dose from `label` (its `amounts` and
    /// `compounds`) and returns its ID.
    async fn supplement(&self, name: &str, label: Value) -> String {
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

    /// Logs `portions` (`[food_id, grams]`) and `doses` (`[supplement_id, doses]`) at noon on
    /// `day`.
    async fn eat(&self, day: &str, portions: &[(i64, f64)], doses: &[(&str, f64)]) {
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

    async fn day(&self, day: &str) -> Value {
        let (status, body) = self
            .send(Method::GET, &format!("/api/nutrition/days/{day}"), None)
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        body
    }

    async fn panel(&self, day: &str) -> Value {
        self.day(day).await["micronutrients"].clone()
    }

    /// One nutrient of the day's panel.
    async fn nutrient(&self, day: &str, key: &str) -> Value {
        nutrient(&self.panel(day).await, key)
    }

    async fn summary(&self, from: &str, to: &str, aggregation: &str) -> Value {
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

fn nutrient(panel: &Value, key: &str) -> Value {
    panel["nutrients"]
        .as_array()
        .unwrap_or_else(|| panic!("no nutrients in {panel}"))
        .iter()
        .find(|nutrient| nutrient["key"] == key)
        .unwrap_or_else(|| panic!("no {key} in {panel}"))
        .clone()
}

/// The Recommended Intake and whether it is an adequate intake.
fn reference(nutrient: &Value) -> (Value, Value) {
    (
        nutrient["recommended_intake"].clone(),
        nutrient["adequate_intake"].clone(),
    )
}

#[tokio::test]
async fn the_panel_is_unavailable_without_the_profile_sex_and_birth_date_or_below_adult_age() {
    let api = Api::start().await;

    let panel = api.panel("2026-04-08").await;
    assert_eq!(panel["state"], "unavailable");
    assert_eq!(panel["missing"], json!(["sex", "birth_date"]));
    assert_eq!(
        panel["reason"],
        "The micronutrient panel needs the Profile's sex (male or female) and birth date"
    );

    api.profile(json!({"sex": "Female"})).await;
    let panel = api.panel("2026-04-08").await;
    assert_eq!(panel["missing"], json!(["birth_date"]));
    assert_eq!(
        panel["reason"],
        "The micronutrient panel needs the Profile's birth date"
    );

    // 17 on the day read: NNR 2023's adult references start at 18.
    api.profile(json!({"birth_date": "2008-06-01"})).await;
    let panel = api.panel("2026-04-08").await;
    assert_eq!(panel["state"], "unavailable");
    assert_eq!(panel["missing"], json!([]));
    assert_eq!(
        panel["reason"],
        "The micronutrient panel covers adults from 18; on 2026-04-08 the Profile's birth date \
         gives 17"
    );

    // The age follows the day read, so the panel starts on the 18th birthday.
    let panel = api.panel("2026-06-01").await;
    assert_eq!(panel["state"], "available");
    assert_eq!(panel["sex"], "female");
    assert_eq!(panel["age"], 18);
    assert_eq!(panel["age_band"], "18-24");
}

#[tokio::test]
async fn sex_and_the_age_band_on_the_day_choose_the_recommended_intakes() {
    let api = Api::start().await;
    api.profile(json!({"sex": "female", "birth_date": "1996-01-15"}))
        .await;

    // A woman of 30: NNR 2023 25-50 years.
    let panel = api.panel("2026-04-08").await;
    assert_eq!(
        (
            panel["state"].clone(),
            panel["age"].clone(),
            panel["age_band"].clone()
        ),
        (json!("available"), json!(30), json!("25-50"))
    );
    assert_eq!(panel["reference_energy_mj"], 9.0);
    assert_eq!(
        reference(&nutrient(&panel, "vitamin_c")),
        (json!(95.0), json!(false))
    );
    assert_eq!(
        reference(&nutrient(&panel, "vitamin_d")),
        (json!(10.0), json!(false))
    );
    assert_eq!(
        reference(&nutrient(&panel, "iron")),
        (json!(15.0), json!(false))
    );
    assert_eq!(
        reference(&nutrient(&panel, "magnesium")),
        (json!(300.0), json!(true))
    );
    // Thiamin and niacin are set per MJ: 0.1 mg and 1.6 mg NE × 9.0 MJ.
    assert_eq!(
        reference(&nutrient(&panel, "vitamin_b1")),
        (json!(0.9), json!(false))
    );
    assert_eq!(
        reference(&nutrient(&panel, "niacin_equivalent")),
        (json!(14.4), json!(false))
    );
    assert_eq!(nutrient(&panel, "vitamin_b1")["unit"], "mg");
    assert_eq!(nutrient(&panel, "niacin_equivalent")["unit"], "mg NE");

    // A man of 30.
    api.profile(json!({"sex": "male"})).await;
    let panel = api.panel("2026-04-08").await;
    assert_eq!(panel["sex"], "male");
    assert_eq!(panel["reference_energy_mj"], 11.3);
    assert_eq!(
        reference(&nutrient(&panel, "vitamin_c")),
        (json!(110.0), json!(false))
    );
    assert_eq!(
        reference(&nutrient(&panel, "iron")),
        (json!(9.0), json!(false))
    );
    assert_eq!(
        reference(&nutrient(&panel, "magnesium")),
        (json!(350.0), json!(true))
    );
    assert_eq!(
        reference(&nutrient(&panel, "vitamin_b1")),
        (json!(1.13), json!(false))
    );

    // A man who turns 71 on 2026-04-10 moves from 51-70 to 71+ years that day.
    api.profile(json!({"birth_date": "1955-04-10"})).await;
    let panel = api.panel("2026-04-09").await;
    assert_eq!(
        (panel["age"].clone(), panel["age_band"].clone()),
        (json!(70), json!("51-70"))
    );
    assert_eq!(
        reference(&nutrient(&panel, "vitamin_d")),
        (json!(10.0), json!(false))
    );
    let panel = api.panel("2026-04-10").await;
    assert_eq!(
        (panel["age"].clone(), panel["age_band"].clone()),
        (json!(71), json!("71+"))
    );
    assert_eq!(panel["reference_energy_mj"], 10.1);
    assert_eq!(
        reference(&nutrient(&panel, "vitamin_d")),
        (json!(20.0), json!(false))
    );
    assert_eq!(
        reference(&nutrient(&panel, "vitamin_a")),
        (json!(750.0), json!(false))
    );
    assert_eq!(
        reference(&nutrient(&panel, "vitamin_b1")),
        (json!(1.01), json!(false))
    );
}

/// A woman of 30 on the days these tests read.
async fn woman(api: &Api) {
    api.profile(json!({"sex": "female", "birth_date": "1996-01-15"}))
        .await;
}

/// The amount, its food and supplement shares, and the percentage of the Recommended Intake.
fn coverage(nutrient: &Value) -> Value {
    json!([
        nutrient["amount"],
        nutrient["food"],
        nutrient["supplements"],
        nutrient["percent_of_recommended"],
    ])
}

/// What each Upper Limit applies to, with the day's amount of it, the trailing average, the days
/// averaged and the status.
fn limits(nutrient: &Value) -> Value {
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

#[tokio::test]
async fn each_nutrient_is_a_percentage_of_its_recommended_intake_split_into_food_and_supplements() {
    let api = Api::start().await;
    woman(&api).await;
    let vitamin_d = api
        .supplement(
            "Vitamin D3",
            json!({"amounts": [{"key": "vitamin_d", "value": 10, "unit": "µg"}]}),
        )
        .await;
    // 200 g of Orange and 100 g of Salmon with one 10 µg vitamin D capsule.
    api.eat(
        "2026-04-08",
        &[(1001, 200.0), (1002, 100.0)],
        &[(&vitamin_d, 1.0)],
    )
    .await;

    let panel = api.panel("2026-04-08").await;
    assert_eq!(
        coverage(&nutrient(&panel, "vitamin_c")),
        json!([100.0, 100.0, null, 105.3])
    );
    assert_eq!(
        coverage(&nutrient(&panel, "vitamin_d")),
        json!([20.0, 10.0, 10.0, 200.0])
    );
    assert_eq!(
        coverage(&nutrient(&panel, "folate")),
        json!([60.0, 60.0, null, 18.2])
    );
    // Neither Food has a vitamin K value: unknown, not zero.
    let vitamin_k = nutrient(&panel, "vitamin_k");
    assert_eq!(coverage(&vitamin_k), json!([null, null, null, null]));
    assert_eq!(vitamin_k["unknown_portions"], 2);
    assert_eq!(vitamin_k["adequate_intake"], true);
    assert_eq!(vitamin_k["source"], "NNR 2023 Table 13");

    // A day without Meal Logs knows no amounts.
    let empty = api.panel("2026-04-09").await;
    assert_eq!(empty["state"], "available");
    assert_eq!(
        coverage(&nutrient(&empty, "vitamin_c")),
        json!([null, null, null, null])
    );
}

#[tokio::test]
async fn a_day_above_an_upper_limit_is_a_caution_and_a_seven_day_average_above_it_says_cut_down() {
    let api = Api::start().await;
    woman(&api).await;
    let strong = api
        .supplement(
            "Vitamin D 150",
            json!({"amounts": [{"key": "vitamin_d", "value": 150, "unit": "µg"}]}),
        )
        .await;
    // Three days of 10 g of Salmon (1 µg vitamin D), then one 150 µg capsule alone.
    for day in ["2026-03-29", "2026-03-30", "2026-03-31"] {
        api.eat(day, &[(1002, 10.0)], &[]).await;
    }
    api.eat("2026-04-01", &[], &[(&strong, 1.0)]).await;

    // Above the 100 µg Upper Limit that day, but the average of the 4 logged days is 38.25 µg.
    let vitamin_d = api.nutrient("2026-04-01", "vitamin_d").await;
    assert_eq!(
        limits(&vitamin_d),
        json!([["total", 100.0, 150.0, 38.25, 4, "caution"]])
    );
    assert_eq!(vitamin_d["upper_limit_status"], "caution");

    // The next day logs nothing: no caution, and it does not count toward the average as zero.
    let vitamin_d = api.nutrient("2026-04-02", "vitamin_d").await;
    assert_eq!(
        limits(&vitamin_d),
        json!([["total", 100.0, null, 38.25, 4, "none"]])
    );

    // Two more 150 µg days average 150 µg over the 7 days to 2026-04-08, but two logged days
    // are too few to call it a habit.
    api.eat("2026-04-05", &[], &[(&strong, 1.0)]).await;
    api.eat("2026-04-07", &[], &[(&strong, 1.0)]).await;
    let vitamin_d = api.nutrient("2026-04-08", "vitamin_d").await;
    assert_eq!(
        limits(&vitamin_d),
        json!([["total", 100.0, null, 150.0, 2, "none"]])
    );

    // A third, with nothing logged between them, averages 150 µg over 3 logged days because
    // untracked days are left out: cut down, even on a day without a dose.
    api.eat("2026-04-06", &[], &[(&strong, 1.0)]).await;
    let panel = api.panel("2026-04-08").await;
    assert_eq!(
        panel["trailing_window"],
        json!({"from": "2026-04-02", "to": "2026-04-08", "logged_days": 3})
    );
    let vitamin_d = nutrient(&panel, "vitamin_d");
    assert_eq!(
        limits(&vitamin_d),
        json!([["total", 100.0, null, 150.0, 3, "cut_down"]])
    );
    assert_eq!(vitamin_d["upper_limit_status"], "cut_down");

    // A day above the limit within a habitual excess says cut down rather than caution: its
    // window reaches back to 2026-04-01.
    let vitamin_d = api.nutrient("2026-04-07", "vitamin_d").await;
    assert_eq!(
        limits(&vitamin_d),
        json!([["total", 100.0, 150.0, 150.0, 4, "cut_down"]])
    );
}

#[tokio::test]
async fn a_single_logged_day_above_an_upper_limit_is_only_a_caution() {
    let api = Api::start().await;
    woman(&api).await;
    let strong = api
        .supplement(
            "Vitamin D 150",
            json!({"amounts": [{"key": "vitamin_d", "value": 150, "unit": "µg"}]}),
        )
        .await;
    api.eat("2026-04-08", &[], &[(&strong, 1.0)]).await;

    // The trailing average is that one day, above the limit, yet one day is not a habit.
    let vitamin_d = api.nutrient("2026-04-08", "vitamin_d").await;
    assert_eq!(
        limits(&vitamin_d),
        json!([["total", 100.0, 150.0, 150.0, 1, "caution"]])
    );
    assert_eq!(vitamin_d["upper_limit_status"], "caution");

    // The summary judges the day the same way: a day above, no cut down.
    let week = api.summary("2026-04-06", "2026-04-12", "week").await;
    assert_eq!(
        limit_flags(&week["periods"][0]["micronutrients"], "vitamin_d"),
        json!([1, false])
    );
}

#[tokio::test]
async fn supplement_only_upper_limits_ignore_food_sources() {
    let api = Api::start().await;
    woman(&api).await;
    // 100 g of Pumpkin seeds (600 mg magnesium, 10 mg NE niacin) and 400 g of Wholemeal bread
    // (1,200 µg folate): far above those Upper Limits, but they apply to supplements only.
    api.eat("2026-04-08", &[(1003, 100.0), (1005, 400.0)], &[])
        .await;

    let panel = api.panel("2026-04-08").await;
    let magnesium = nutrient(&panel, "magnesium");
    assert_eq!(magnesium["amount"], 600.0);
    assert_eq!(
        limits(&magnesium),
        json!([["supplements", 250.0, 0.0, 0.0, 1, "none"]])
    );
    let folate = nutrient(&panel, "folate");
    assert_eq!(folate["amount"], 1200.0);
    assert_eq!(
        limits(&folate),
        json!([["supplemental_folate", 1000.0, 0.0, 0.0, 1, "none"]])
    );
    assert_eq!(
        limits(&nutrient(&panel, "niacin_equivalent")),
        json!([
            ["nicotinic_acid", 10.0, 0.0, 0.0, 1, "none"],
            ["nicotinamide", 900.0, 0.0, 0.0, 1, "none"],
        ])
    );

    // The next day adds a magnesium, a folic acid and a nicotinic acid supplement.
    let magnesium = api
        .supplement(
            "Magnesium",
            json!({"amounts": [{"key": "magnesium", "value": 300, "unit": "mg"}]}),
        )
        .await;
    let folic_acid = api
        .supplement(
            "Folic acid",
            json!({"amounts": [
                {"key": "folate", "form": "folic_acid", "value": 1100, "unit": "µg"},
            ]}),
        )
        .await;
    let niacin = api
        .supplement(
            "Niacin",
            json!({"amounts": [
                {"key": "niacin_equivalent", "form": "nicotinic_acid", "value": 20, "unit": "mg"},
            ]}),
        )
        .await;
    api.eat(
        "2026-04-09",
        &[(1003, 100.0)],
        &[(&magnesium, 1.0), (&folic_acid, 1.0), (&niacin, 1.0)],
    )
    .await;

    let panel = api.panel("2026-04-09").await;
    let magnesium = nutrient(&panel, "magnesium");
    assert_eq!(coverage(&magnesium), json!([900.0, 600.0, 300.0, 300.0]));
    assert_eq!(
        limits(&magnesium),
        json!([["supplements", 250.0, 300.0, 150.0, 2, "caution"]])
    );
    assert_eq!(
        limits(&nutrient(&panel, "folate")),
        json!([["supplemental_folate", 1000.0, 1100.0, 550.0, 2, "caution"]])
    );
    // Each niacin form has its own limit: nicotinamide stays clear.
    let niacin = nutrient(&panel, "niacin_equivalent");
    assert_eq!(
        limits(&niacin),
        json!([
            ["nicotinic_acid", 10.0, 20.0, 10.0, 2, "caution"],
            ["nicotinamide", 900.0, 0.0, 0.0, 2, "none"],
        ])
    );
    assert_eq!(niacin["upper_limit_status"], "caution");
}

#[tokio::test]
async fn preformed_vitamin_a_counts_retinol_from_food_and_supplements_but_not_beta_carotene() {
    let api = Api::start().await;
    woman(&api).await;
    let beta_carotene = api
        .supplement(
            "Beta-carotene",
            json!({"amounts": [
                {"key": "vitamin_a", "form": "beta_carotene", "value": 2000, "unit": "µg"},
            ]}),
        )
        .await;
    let retinol = api
        .supplement(
            "Retinol",
            json!({"amounts": [
                {"key": "vitamin_a", "form": "retinol", "value": 1500, "unit": "µg"},
            ]}),
        )
        .await;
    // 50 g of Liver pate is 2,000 µg RE, all of it retinol.
    api.eat("2026-05-20", &[(1004, 50.0)], &[(&beta_carotene, 1.0)])
        .await;
    api.eat("2026-05-21", &[(1004, 50.0)], &[(&retinol, 1.0)])
        .await;

    let vitamin_a = api.nutrient("2026-05-20", "vitamin_a").await;
    assert_eq!(coverage(&vitamin_a), json!([4000.0, 2000.0, 2000.0, 571.4]));
    assert_eq!(
        limits(&vitamin_a),
        json!([["preformed_vitamin_a", 3000.0, 2000.0, 2000.0, 1, "none"]])
    );
    let vitamin_a = api.nutrient("2026-05-21", "vitamin_a").await;
    assert_eq!(
        limits(&vitamin_a),
        json!([["preformed_vitamin_a", 3000.0, 3500.0, 2750.0, 2, "caution"]])
    );
}

#[tokio::test]
async fn nutrients_without_an_upper_limit_are_never_flagged() {
    let api = Api::start().await;
    woman(&api).await;
    let vitamin_c = api
        .supplement(
            "Vitamin C",
            json!({"amounts": [{"key": "vitamin_c", "value": 1000, "unit": "mg"}]}),
        )
        .await;
    for day in ["2026-04-06", "2026-04-07", "2026-04-08"] {
        api.eat(day, &[(1002, 100.0)], &[(&vitamin_c, 2.0)]).await;
    }

    let panel = api.panel("2026-04-08").await;
    let vitamin_c = nutrient(&panel, "vitamin_c");
    assert_eq!(vitamin_c["percent_of_recommended"], 2105.3);
    assert_eq!(vitamin_c["upper_limits"], json!([]));
    assert_eq!(vitamin_c["upper_limit_status"], Value::Null);
    // Vitamin D has a limit the day stays under.
    assert_eq!(nutrient(&panel, "vitamin_d")["upper_limit_status"], "none");
}

#[tokio::test]
async fn epa_and_dha_from_food_and_supplements_add_up_against_the_efsa_adequate_intake() {
    let api = Api::start().await;
    woman(&api).await;
    let fish_oil = api
        .supplement(
            "Fish oil",
            json!({
                "amounts": [
                    {"key": "epa", "value": 180, "unit": "mg"},
                    {"key": "dha", "value": 120, "unit": "mg"},
                ],
                "compounds": [{"name": "Creatine monohydrate", "value": 3, "unit": "g"}],
            }),
        )
        .await;
    // 10 g of Salmon: 50 mg EPA and 120 mg DHA; one capsule: 180 mg EPA and 120 mg DHA.
    api.eat("2026-04-08", &[(1002, 10.0)], &[(&fish_oil, 1.0)])
        .await;

    let day = api.day("2026-04-08").await;
    let panel = &day["micronutrients"];
    let omega_3 = nutrient(panel, "epa_dha");
    assert_eq!(
        json!([
            omega_3["name"],
            omega_3["unit"],
            omega_3["recommended_intake"],
            omega_3["adequate_intake"],
            omega_3["upper_limits"],
            omega_3["upper_limit_status"],
        ]),
        json!(["EPA+DHA", "mg", 250.0, true, [], null])
    );
    assert_eq!(coverage(&omega_3), json!([470.0, 170.0, 300.0, 188.0]));

    // A compound without a reference is an amount taken, outside the panel.
    assert_eq!(
        day["compounds"],
        json!([{"name": "Creatine monohydrate", "value": 3.0, "unit": "g"}])
    );
    let keys: Vec<&Value> = panel["nutrients"]
        .as_array()
        .unwrap()
        .iter()
        .map(|nutrient| &nutrient["key"])
        .collect();
    assert!(!keys.contains(&&json!("creatine_monohydrate")), "{keys:?}");
}

/// One nutrient's coverage: the mean percentage of its Recommended Intake, the days averaged, the
/// days above an Upper Limit and whether a trailing 7-day average was above one.
fn period_coverage(coverage: &Value, key: &str) -> Value {
    let nutrient = nutrient(coverage, key);
    json!([
        nutrient["percent_of_recommended"],
        nutrient["averaged_days"],
        nutrient["days_above_upper_limit"],
        nutrient["seven_day_average_above_upper_limit"],
    ])
}

/// One nutrient's days above an Upper Limit and whether a trailing 7-day average was above one.
fn limit_flags(coverage: &Value, key: &str) -> Value {
    let nutrient = nutrient(coverage, key);
    json!([
        nutrient["days_above_upper_limit"],
        nutrient["seven_day_average_above_upper_limit"],
    ])
}

#[tokio::test]
async fn summary_coverage_averages_the_recommended_intake_over_logged_days_only() {
    let api = Api::start().await;
    woman(&api).await;
    let vitamin_d = api
        .supplement(
            "Vitamin D3",
            json!({"amounts": [{"key": "vitamin_d", "value": 20, "unit": "µg"}]}),
        )
        .await;
    // Monday 200 g of Orange (100 mg vitamin C, 105.3 %) and Wednesday 100 g (50 mg, 52.6 %).
    // Friday logs only a 20 µg vitamin D capsule: a logged day without known energy.
    api.eat("2026-04-06", &[(1001, 200.0)], &[]).await;
    api.eat("2026-04-08", &[(1001, 100.0)], &[]).await;
    api.eat("2026-04-10", &[], &[(&vitamin_d, 1.0)]).await;

    let week = api.summary("2026-04-06", "2026-04-12", "week").await;
    let period = &week["periods"][0];
    assert_eq!(
        json!([
            period["logged_days"],
            period["missing_days"],
            period["intake_days"]
        ]),
        json!([3, 4, 2])
    );
    // Energy is the mean over the days that know it (94 and 47 kcal), so the capsule day is not
    // a day of 0 kcal.
    assert_eq!(period["energy"]["eaten_kcal"], 71.0);
    let coverage = &period["micronutrients"];
    assert_eq!(coverage["logged_days"], 3);
    // The four days without Meal Logs, and Friday's unknown vitamin C, leave the mean alone
    // instead of counting as zero.
    assert_eq!(
        period_coverage(coverage, "vitamin_c"),
        json!([78.9, 2, null, null])
    );
    assert_eq!(
        period_coverage(coverage, "folate"),
        json!([13.6, 2, 0, false])
    );
    // No Orange has a vitamin K value: unknown, not zero.
    assert_eq!(
        period_coverage(coverage, "vitamin_k"),
        json!([null, 0, null, null])
    );
    // Friday's capsule is the only known vitamin D: 20 µg of the 10 µg Recommended Intake.
    assert_eq!(
        period_coverage(coverage, "vitamin_d"),
        json!([200.0, 1, 0, false])
    );

    // By day, a day without Meal Logs has no coverage.
    let days = api.summary("2026-04-06", "2026-04-08", "day").await;
    let percents: Vec<Value> = days["periods"]
        .as_array()
        .unwrap()
        .iter()
        .map(|period| {
            json!([
                period["micronutrients"]["logged_days"],
                nutrient(&period["micronutrients"], "vitamin_c")["percent_of_recommended"],
            ])
        })
        .collect();
    assert_eq!(
        percents,
        vec![json!([1, 105.3]), json!([0, null]), json!([1, 52.6])]
    );

    // The range carries the same means beside each nutrient's reference.
    let range = &week["micronutrients"];
    assert_eq!(range["state"], "available");
    assert_eq!(range["logged_days"], 3);
    let vitamin_c = nutrient(range, "vitamin_c");
    assert_eq!(
        json!([
            vitamin_c["name"],
            vitamin_c["group"],
            vitamin_c["unit"],
            vitamin_c["adequate_intake"],
            vitamin_c["percent_of_recommended"],
            vitamin_c["averaged_days"],
        ]),
        json!(["Vitamin C", "vitamins", "mg", false, 78.9, 2])
    );
    assert_eq!(nutrient(range, "epa_dha")["unit"], "mg");
}

#[tokio::test]
async fn a_lasting_shortfall_is_a_range_mean_below_70_percent_over_at_least_7_logged_days() {
    let api = Api::start().await;
    woman(&api).await;
    // 100 g of Orange a day: 50 mg vitamin C (52.6 %) and 30 µg folate (9.1 %).
    let days = [
        "2026-04-01",
        "2026-04-02",
        "2026-04-03",
        "2026-04-05",
        "2026-04-06",
        "2026-04-08",
    ];
    for day in days {
        api.eat(day, &[(1001, 100.0)], &[]).await;
    }

    let range = api.summary("2026-04-01", "2026-04-10", "week").await["micronutrients"].clone();
    assert_eq!(
        range["shortfall"],
        json!({"below_percent": 70.0, "min_logged_days": 7})
    );
    // Six logged days are too few to call it lasting.
    assert_eq!(nutrient(&range, "vitamin_c")["lasting_shortfall"], false);

    api.eat("2026-04-09", &[(1001, 100.0)], &[]).await;
    let range = api.summary("2026-04-01", "2026-04-10", "week").await["micronutrients"].clone();
    let shortfall = |key| {
        let nutrient = nutrient(&range, key);
        json!([
            nutrient["percent_of_recommended"],
            nutrient["lasting_shortfall"]
        ])
    };
    assert_eq!(shortfall("vitamin_c"), json!([52.6, true]));
    assert_eq!(shortfall("folate"), json!([9.1, true]));
    // Unknown is never a shortfall.
    assert_eq!(shortfall("vitamin_d"), json!([null, false]));

    // 900 g on an eighth day (450 mg) lifts the mean above the threshold.
    api.eat("2026-04-10", &[(1001, 900.0)], &[]).await;
    let range = api.summary("2026-04-01", "2026-04-10", "week").await["micronutrients"].clone();
    let vitamin_c = nutrient(&range, "vitamin_c");
    assert_eq!(
        json!([
            vitamin_c["percent_of_recommended"],
            vitamin_c["lasting_shortfall"]
        ]),
        json!([105.3, false])
    );
}

#[tokio::test]
async fn summary_counts_days_above_upper_limits_and_cut_down_windows_as_the_day_view_does() {
    let api = Api::start().await;
    woman(&api).await;
    let strong = api
        .supplement(
            "Vitamin D 150",
            json!({"amounts": [{"key": "vitamin_d", "value": 150, "unit": "µg"}]}),
        )
        .await;
    let magnesium = api
        .supplement(
            "Magnesium",
            json!({"amounts": [{"key": "magnesium", "value": 300, "unit": "mg"}]}),
        )
        .await;
    // Week of 6 April: 10 g of Salmon (1 µg vitamin D) on Monday, a 150 µg capsule alone on
    // Tuesday and Thursday, and 100 g of Pumpkin seeds (600 mg magnesium from food) on Wednesday.
    api.eat("2026-04-06", &[(1002, 10.0)], &[]).await;
    api.eat("2026-04-07", &[], &[(&strong, 1.0)]).await;
    api.eat("2026-04-08", &[(1003, 100.0)], &[]).await;
    api.eat("2026-04-09", &[], &[(&strong, 1.0)]).await;
    // Week of 13 April: Pumpkin seeds with a 300 mg magnesium capsule on Wednesday.
    api.eat("2026-04-15", &[(1003, 100.0)], &[(&magnesium, 1.0)])
        .await;
    // The Day view agrees: Tuesday is a caution, Thursday's average of 100.3 µg is a cut down.
    assert_eq!(
        api.nutrient("2026-04-07", "vitamin_d").await["upper_limit_status"],
        "caution"
    );
    assert_eq!(
        api.nutrient("2026-04-09", "vitamin_d").await["upper_limit_status"],
        "cut_down"
    );

    let summary = api.summary("2026-04-06", "2026-04-26", "week").await;
    let weeks: Vec<Value> = summary["periods"]
        .as_array()
        .unwrap()
        .iter()
        .map(|period| {
            let coverage = &period["micronutrients"];
            json!([
                period["start"],
                limit_flags(coverage, "vitamin_d"),
                limit_flags(coverage, "magnesium"),
            ])
        })
        .collect();
    assert_eq!(
        weeks,
        vec![
            // Two capsule days above 100 µg, and Thursday's window averages above it. The 600 mg
            // of magnesium from food is not checked against the supplements-only limit.
            json!(["2026-04-06", [2, true], [0, false]]),
            // No day above: Wednesday's window reaches back to Thursday's capsule, but that is
            // its only known vitamin D, too few days to cut down. The 300 mg capsule is above the
            // 250 mg supplement limit for a day only.
            json!(["2026-04-13", [0, false], [1, false]]),
            // Nothing logged: never flagged.
            json!(["2026-04-20", [0, false], [0, false]]),
        ]
    );

    let range = &summary["micronutrients"];
    assert_eq!(limit_flags(range, "vitamin_d"), json!([2, true]));
    assert_eq!(limit_flags(range, "magnesium"), json!([1, false]));
    // Without an Upper Limit there is nothing to count.
    assert_eq!(limit_flags(range, "vitamin_c"), json!([null, null]));
}

#[tokio::test]
async fn summary_coverage_is_unavailable_with_the_reason_while_the_rest_still_works() {
    let api = Api::start().await;
    api.eat("2026-04-08", &[(1001, 200.0)], &[]).await;

    let summary = api.summary("2026-04-06", "2026-04-12", "week").await;
    assert_eq!(
        summary["micronutrients"],
        json!({
            "state": "unavailable",
            "missing": ["sex", "birth_date"],
            "reason": "The micronutrient panel needs the Profile's sex (male or female) and birth date",
        })
    );
    let period = &summary["periods"][0];
    assert_eq!(period["micronutrients"], Value::Null);
    assert_eq!(period["energy"]["eaten_kcal"], 94.0);

    // Under 18 for the whole range.
    api.profile(json!({"sex": "female", "birth_date": "2010-01-01"}))
        .await;
    let summary = api.summary("2026-04-06", "2026-04-12", "week").await;
    assert_eq!(summary["micronutrients"]["state"], "unavailable");
    assert_eq!(summary["micronutrients"]["missing"], json!([]));
    assert_eq!(
        summary["micronutrients"]["reason"],
        "The micronutrient panel covers adults from 18; on 2026-04-12 the Profile's birth date \
         gives 16"
    );
    assert_eq!(summary["periods"][0]["micronutrients"], Value::Null);
}
