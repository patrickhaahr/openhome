//! The Food database through the nutrition routes: a synthetic workbook shaped like DTU's Danish
//! Food Composition Database is served by `wiremock` and imported into a real `food.db`.

mod common;

use std::fmt::Write;
use std::path::PathBuf;

use axum::{Router, http::StatusCode};
use common::{send_request, test_app_with_db};
use openhome_api::{
    auth::{ApiKey, auth_middleware},
    db, routes,
    services::nutrition::food_database::{DatasetPin, FoodDatabase, FoodImport},
};
use pretty_assertions::assert_eq;
use rust_xlsxwriter::Workbook;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tempfile::TempDir;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};

const API_KEY: &str = "test-api-key";

struct SyntheticParameter {
    id: i64,
    name_da: &'static str,
    name_en: &'static str,
    unit: &'static str,
    sort_key: i64,
    group_da: &'static str,
    group_en: &'static str,
    /// Glycerol is defined on the Parameter sheet but, as in DTU's workbook, has no data column.
    has_column: bool,
}

const fn parameter(
    id: i64,
    names: (&'static str, &'static str),
    unit: &'static str,
    sort_key: i64,
    group: (&'static str, &'static str),
) -> SyntheticParameter {
    SyntheticParameter {
        id,
        name_da: names.0,
        name_en: names.1,
        unit,
        sort_key,
        group_da: group.0,
        group_en: group.1,
        has_column: true,
    }
}

const MACRO: (&str, &str) = ("Makronæringstoffer m.m.", "Macro nutrients etc.");
const FACTORS: (&str, &str) = ("Faktorer m.m.", "Factors etc.");

fn parameters(protein_unit: &'static str) -> Vec<SyntheticParameter> {
    vec![
        parameter(137, ("Energi (kJ)", "Energy (kJ)"), "kJ/100g", 1010, MACRO),
        parameter(
            356,
            ("Energi (kcal)", "Energy (kcal)"),
            "kcal/100 g",
            1030,
            MACRO,
        ),
        parameter(218, ("Protein", "Protein"), protein_unit, 1110, MACRO),
        parameter(
            172,
            ("Tilgængelig kulhydrat", "Available carbohydrates"),
            "g/100g",
            1220,
            MACRO,
        ),
        parameter(168, ("Kostfibre", "Dietary fibre"), "g/100g", 1240, MACRO),
        parameter(141, ("Fedt", "Fat"), "g/100g", 1310, MACRO),
        parameter(19, ("Alkohol", "Alcohol"), "g/100g", 1410, MACRO),
        parameter(
            126,
            ("D-vitamin", "Vitamin D"),
            "µg/100g",
            2110,
            ("Vitaminer", "Vitamins"),
        ),
        SyntheticParameter {
            has_column: false,
            ..parameter(
                447,
                ("Glycerin", "Glycerol"),
                "g/100g",
                4580,
                ("Sukkeralkoholer", "Sugar alcohols"),
            )
        },
        parameter(
            87,
            ("C20:5,n-3", "C20:5,n-3"),
            "g/100g",
            9110,
            ("Flerumættede fedtsyrer", "Polyunsaturated fatty acids"),
        ),
        parameter(252, ("Svind", "Waste"), "%", 20010, FACTORS),
        parameter(363, ("Massefylde", "Density"), "g/ml", 20210, FACTORS),
        parameter(432, ("pH", "pH"), "No Unit", 20220, FACTORS),
    ]
}

struct SyntheticFood {
    id: i64,
    name_da: &'static str,
    name_en: &'static str,
    group: (i64, &'static str, &'static str),
    /// `ParameterID` and value; an absent parameter is an empty cell.
    values: Vec<(i64, f64)>,
}

fn skyr(protein: f64) -> SyntheticFood {
    SyntheticFood {
        id: 1693,
        name_da: "Skyr, 0.2 % fedt",
        name_en: "Skyr, 0.2 % fat",
        group: (3, "Syrnede mælkeprodukter", "Fermented milk products"),
        values: vec![
            (137, 260.0),
            (356, 62.0),
            (218, protein),
            (172, 4.0),
            (168, 0.0),
            (141, 0.2),
            (19, 0.0),
            (126, 0.0),
            (252, 0.0),
            (363, 1.03),
            (432, 4.2),
        ],
    }
}

fn chicken_breast() -> SyntheticFood {
    SyntheticFood {
        id: 907,
        name_da: "Kylling, bryst, kød og skind, rå",
        name_en: "Chicken, breast, flesh and skin, raw",
        group: (80, "Fjerkræ", "Poultry"),
        values: vec![
            (356, 150.0),
            (218, 21.0),
            (141, 7.0),
            (87, 0.01),
            (252, 10.0),
        ],
    }
}

fn banana() -> SyntheticFood {
    SyntheticFood {
        id: 3,
        name_da: "Banan, rå",
        name_en: "Banana, raw",
        group: (52, "Sydfrugt", "Tropical or subtropical fruit"),
        values: vec![(356, 90.0), (172, 20.0), (252, 40.0)],
    }
}

fn apple() -> SyntheticFood {
    SyntheticFood {
        id: 2,
        name_da: "Æble, uspec., råt",
        name_en: "Apple, raw, all varieties",
        group: (49, "Kernefrugt", "Pome fruit"),
        values: vec![(356, 50.0), (172, 11.0), (252, 12.0)],
    }
}

fn rye_bread() -> SyntheticFood {
    SyntheticFood {
        id: 5,
        name_da: "Rugbrød, fuldkorn",
        name_en: "Rye bread, wholemeal",
        group: (27, "Brød", "Bread"),
        values: vec![(356, 210.0), (218, 6.0)],
    }
}

fn first_dataset() -> Vec<u8> {
    workbook(
        &parameters("g/100g"),
        &[apple(), banana(), chicken_breast(), skyr(11.0)],
    )
}

/// A later dataset: Skyr's protein changes, Banana is dropped and Rye bread is new.
fn second_dataset() -> Vec<u8> {
    workbook(
        &parameters("g/100g"),
        &[apple(), chicken_breast(), skyr(12.0), rye_bread()],
    )
}

/// The sheets and header layout of DTU's `FCDB_6.1_Dataset.xlsx`.
fn workbook(parameters: &[SyntheticParameter], foods: &[SyntheticFood]) -> Vec<u8> {
    let mut book = Workbook::new();
    book.add_worksheet()
        .set_name("Readme")
        .unwrap()
        .write_string(0, 0, "Synthetic workbook for tests")
        .unwrap();

    let table = book.add_worksheet().set_name("Data_Table").unwrap();
    let columns: Vec<&SyntheticParameter> = parameters.iter().filter(|p| p.has_column).collect();
    for (row, label) in ["→ParameterNavn", "→ParameterName", "→Unit"]
        .into_iter()
        .enumerate()
    {
        table.write_string(row as u32, 2, label).unwrap();
    }
    table.write_string(3, 0, "↓FødevareNavn").unwrap();
    table.write_string(3, 1, "↓FoodName").unwrap();
    table.write_string(3, 2, "↓FoodID/→ParameterID").unwrap();
    for (index, parameter) in columns.iter().enumerate() {
        let column = 3 + index as u16;
        table.write_string(0, column, parameter.name_da).unwrap();
        table.write_string(1, column, parameter.name_en).unwrap();
        table.write_string(2, column, parameter.unit).unwrap();
        table.write_number(3, column, parameter.id as f64).unwrap();
    }
    let trailing = 3 + columns.len() as u16;
    for (offset, label) in ["↓FoodGroupID", "↓FødevareGruppe", "↓FoodGroup"]
        .into_iter()
        .enumerate()
    {
        table
            .write_string(3, trailing + offset as u16, label)
            .unwrap();
    }
    for (index, food) in foods.iter().enumerate() {
        let row = 4 + index as u32;
        table.write_string(row, 0, food.name_da).unwrap();
        table.write_string(row, 1, food.name_en).unwrap();
        table.write_number(row, 2, food.id as f64).unwrap();
        for (column, parameter) in columns.iter().enumerate() {
            if let Some((_, value)) = food.values.iter().find(|(id, _)| *id == parameter.id) {
                table.write_number(row, 3 + column as u16, *value).unwrap();
            }
        }
        table
            .write_number(row, trailing, food.group.0 as f64)
            .unwrap();
        table.write_string(row, trailing + 1, food.group.1).unwrap();
        table.write_string(row, trailing + 2, food.group.2).unwrap();
    }

    let food_sheet = book.add_worksheet().set_name("Food").unwrap();
    for (column, header) in [
        "FødevareNavn",
        "FoodName",
        "FoodID",
        "TaxonomicName",
        "FoodGroupID",
        "FødevareGruppe",
        "FoodGroup",
    ]
    .into_iter()
    .enumerate()
    {
        food_sheet.write_string(0, column as u16, header).unwrap();
    }
    for (index, food) in foods.iter().enumerate() {
        let row = 1 + index as u32;
        food_sheet.write_string(row, 0, food.name_da).unwrap();
        food_sheet.write_string(row, 1, food.name_en).unwrap();
        food_sheet.write_number(row, 2, food.id as f64).unwrap();
        food_sheet.write_string(row, 3, "NULL").unwrap();
        food_sheet
            .write_number(row, 4, food.group.0 as f64)
            .unwrap();
        food_sheet.write_string(row, 5, food.group.1).unwrap();
        food_sheet.write_string(row, 6, food.group.2).unwrap();
    }

    let parameter_sheet = book.add_worksheet().set_name("Parameter").unwrap();
    for (column, header) in [
        "ParameterNavn",
        "ParameterName",
        "Unit",
        "SortKey",
        "ParameterID",
        "ParameterGroupID",
        "ParameterGruppeNavn",
        "ParameterGroupName",
    ]
    .into_iter()
    .enumerate()
    {
        parameter_sheet
            .write_string(0, column as u16, header)
            .unwrap();
    }
    for (index, parameter) in parameters.iter().enumerate() {
        let row = 1 + index as u32;
        parameter_sheet
            .write_string(row, 0, parameter.name_da)
            .unwrap();
        parameter_sheet
            .write_string(row, 1, parameter.name_en)
            .unwrap();
        parameter_sheet
            .write_string(row, 2, parameter.unit)
            .unwrap();
        parameter_sheet
            .write_number(row, 3, parameter.sort_key as f64)
            .unwrap();
        parameter_sheet
            .write_number(row, 4, parameter.id as f64)
            .unwrap();
        parameter_sheet.write_number(row, 5, 1.0).unwrap();
        parameter_sheet
            .write_string(row, 6, parameter.group_da)
            .unwrap();
        parameter_sheet
            .write_string(row, 7, parameter.group_en)
            .unwrap();
    }

    book.save_to_buffer().unwrap()
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .fold(String::new(), |mut text, byte| {
            write!(text, "{byte:02x}").unwrap();
            text
        })
}

/// Serves `bytes` at `/files/{version}.xlsx`, expecting `calls` downloads.
async fn serve(server: &MockServer, version: &str, bytes: &[u8], calls: u64) {
    Mock::given(method("GET"))
        .and(path(format!("/files/{version}.xlsx")))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(bytes.to_vec()))
        .expect(calls)
        .mount(server)
        .await;
}

fn pin(server: &MockServer, version: &str, sha256: String) -> DatasetPin {
    DatasetPin {
        version: version.into(),
        url: format!("{}/files/{version}.xlsx", server.uri())
            .parse()
            .unwrap(),
        sha256,
    }
}

fn cache_dir(dir: &TempDir) -> PathBuf {
    dir.path().join("food-dataset")
}

/// Opens `food.db` in `dir` with the shared settings, as the API does on startup.
async fn open(dir: &TempDir, file: &str, pin: DatasetPin) -> (Router, Option<FoodImport>) {
    let url = format!("sqlite:{}", dir.path().join(file).display());
    let pool = db::connect_sqlite(&url).await.unwrap();
    let (foods, import) = FoodDatabase::open(pool, pin, cache_dir(dir)).await.unwrap();
    let (_, state) = test_app_with_db().await;
    let app = routes::nutrition::router(foods)
        .with_state(state)
        .layer(axum::middleware::from_fn(|request, next| {
            auth_middleware(request, next, ApiKey::new(API_KEY.into()))
        }));
    (app, import)
}

/// Opens `food.db` and runs the import it asks for.
async fn imported(dir: &TempDir, pin: DatasetPin) -> Router {
    let (app, import) = open(dir, "food.db", pin).await;
    import.expect("the pin is not recorded yet").run().await;
    app
}

async fn get(app: &Router, uri: &str) -> (StatusCode, Value) {
    send_request(app.clone(), uri, Some(API_KEY)).await
}

fn parameter_value<'a>(food: &'a Value, key: &str) -> &'a Value {
    food["parameters"]
        .as_array()
        .unwrap()
        .iter()
        .find(|parameter| parameter["key"] == key)
        .unwrap_or_else(|| panic!("{key} is missing"))
}

fn cached_files(dir: &TempDir) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(cache_dir(dir)) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

fn assert_unavailable(body: &Value, state: &str) {
    assert_eq!(body["code"], "food_database_unavailable", "{body}");
    assert_eq!(body["state"], state, "{body}");
    assert_eq!(body["status"], 503);
}

#[tokio::test]
async fn first_import_reports_unavailable_until_it_completes_then_keeps_every_value() {
    let server = MockServer::start().await;
    let bytes = first_dataset();
    // DTU Data answers its download URL with a redirect to its object store.
    Mock::given(method("GET"))
        .and(path("/dtu/6.1"))
        .respond_with(
            ResponseTemplate::new(302)
                .insert_header("location", format!("{}/files/6.1.xlsx", server.uri())),
        )
        .expect(1)
        .mount(&server)
        .await;
    serve(&server, "6.1", &bytes, 1).await;
    let dir = TempDir::new().unwrap();
    let pin = DatasetPin {
        url: format!("{}/dtu/6.1", server.uri()).parse().unwrap(),
        ..pin(&server, "6.1", sha256(&bytes))
    };

    let (app, import) = open(&dir, "food.db", pin).await;
    for uri in [
        "/api/nutrition/foods?query=skyr",
        "/api/nutrition/foods/1693",
    ] {
        let (status, body) = get(&app, uri).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{uri}");
        assert_unavailable(&body, "importing");
    }

    import.unwrap().run().await;

    let (status, food) = get(&app, "/api/nutrition/foods/1693").await;
    assert_eq!(status, StatusCode::OK, "{food}");
    assert_eq!(food["food_id"], 1693);
    assert_eq!(food["name_da"], "Skyr, 0.2 % fedt");
    assert_eq!(food["name_en"], "Skyr, 0.2 % fat");
    assert_eq!(
        food["food_group"],
        json!({"id": 3, "name_da": "Syrnede mælkeprodukter", "name_en": "Fermented milk products"})
    );
    assert_eq!(food["density"], json!({"value": 1.03, "unit": "g/ml"}));
    assert_eq!(food["waste"], json!({"value": 0.0, "unit": "%"}));
    let keys: Vec<&str> = food["parameters"]
        .as_array()
        .unwrap()
        .iter()
        .map(|parameter| parameter["key"].as_str().unwrap())
        .collect();
    assert_eq!(
        keys,
        [
            "energy_kj",
            "energy_kcal",
            "protein",
            "available_carbohydrate",
            "dietary_fibre",
            "fat",
            "alcohol",
            "vitamin_d",
            "glycerol",
            "epa",
            "waste",
            "density",
            "ph"
        ]
    );
    assert_eq!(
        parameter_value(&food, "protein"),
        &json!({
            "key": "protein", "parameter_id": 218, "name_da": "Protein", "name_en": "Protein",
            "group": {"name_da": "Makronæringstoffer m.m.", "name_en": "Macro nutrients etc."},
            "unit": "g", "per_100g": true, "value": 11.0
        })
    );
    // A recorded zero stays zero; an empty cell, or a parameter without a column, is unknown.
    assert_eq!(parameter_value(&food, "vitamin_d")["value"], 0.0);
    assert_eq!(parameter_value(&food, "vitamin_d")["unit"], "µg");
    assert_eq!(parameter_value(&food, "epa")["value"], Value::Null);
    assert_eq!(parameter_value(&food, "glycerol")["value"], Value::Null);
    assert_eq!(parameter_value(&food, "energy_kcal")["unit"], "kcal");
    assert_eq!(
        parameter_value(&food, "ph"),
        &json!({
            "key": "ph", "parameter_id": 432, "name_da": "pH", "name_en": "pH",
            "group": {"name_da": "Faktorer m.m.", "name_en": "Factors etc."},
            "unit": null, "per_100g": false, "value": 4.2
        })
    );
    assert_eq!(
        food["source"],
        json!({
            "name": "The Danish Food Composition Database",
            "version": "6.1",
            "credit": "© The Danish Food Composition Database, version 6.1",
            "licence": "CC BY 4.0"
        })
    );

    let (_, chicken) = get(&app, "/api/nutrition/foods/907").await;
    assert_eq!(chicken["density"], json!({"value": null, "unit": "g/ml"}));
    assert_eq!(chicken["waste"], json!({"value": 10.0, "unit": "%"}));
    assert_eq!(cached_files(&dir), ["dtu-fcdb-6.1.xlsx"]);
}

#[tokio::test]
async fn search_matches_danish_and_english_names_with_key_values_density_and_waste() {
    let server = MockServer::start().await;
    let bytes = first_dataset();
    serve(&server, "6.1", &bytes, 1).await;
    let dir = TempDir::new().unwrap();
    let app = imported(&dir, pin(&server, "6.1", sha256(&bytes))).await;

    let (status, body) = get(&app, "/api/nutrition/foods?query=skyr").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["query"], "skyr");
    assert_eq!(body["source"]["version"], "6.1");
    assert_eq!(
        body["foods"],
        json!([{
            "food_id": 1693,
            "name_da": "Skyr, 0.2 % fedt",
            "name_en": "Skyr, 0.2 % fat",
            "food_group": {"id": 3, "name_da": "Syrnede mælkeprodukter", "name_en": "Fermented milk products"},
            "per_100g": {
                "energy_kcal": {"value": 62.0, "unit": "kcal"},
                "energy_kj": {"value": 260.0, "unit": "kJ"},
                "protein": {"value": 11.0, "unit": "g"},
                "fat": {"value": 0.2, "unit": "g"},
                "available_carbohydrate": {"value": 4.0, "unit": "g"},
                "sugars": {"value": null, "unit": "g"},
                "dietary_fibre": {"value": 0.0, "unit": "g"},
                "alcohol": {"value": 0.0, "unit": "g"}
            },
            "density": {"value": 1.03, "unit": "g/ml"},
            "waste": {"value": 0.0, "unit": "%"}
        }])
    );

    for (query, food_id) in [
        ("kyllingebryst", 907),
        ("kylling bryst", 907),
        ("chicken breast", 907),
        ("Chicken, BREAST", 907),
        ("ÆBLE", 2),
        ("apple", 2),
        ("banan", 3),
    ] {
        let uri = format!(
            "/api/nutrition/foods?query={}",
            url::form_urlencoded::byte_serialize(query.as_bytes()).collect::<String>()
        );
        let (status, body) = get(&app, &uri).await;
        assert_eq!(status, StatusCode::OK, "{query}: {body}");
        assert_eq!(body["foods"][0]["food_id"], food_id, "{query}: {body}");
    }

    let (_, body) = get(&app, "/api/nutrition/foods?query=r%C3%A5&limit=2").await;
    assert_eq!(body["foods"].as_array().unwrap().len(), 2, "{body}");
    let (status, body) = get(&app, "/api/nutrition/foods?query=lasagne").await;
    assert_eq!((status, &body["foods"]), (StatusCode::OK, &json!([])));
}

#[tokio::test]
async fn lookups_reject_invalid_requests_and_unknown_foods() {
    let server = MockServer::start().await;
    let bytes = first_dataset();
    serve(&server, "6.1", &bytes, 1).await;
    let dir = TempDir::new().unwrap();
    let app = imported(&dir, pin(&server, "6.1", sha256(&bytes))).await;

    for uri in [
        "/api/nutrition/foods",
        "/api/nutrition/foods?query=%20",
        "/api/nutrition/foods?query=%25%25",
        "/api/nutrition/foods?query=skyr&limit=0",
        "/api/nutrition/foods?query=skyr&limit=51",
        "/api/nutrition/foods?query=skyr&limit=many",
        "/api/nutrition/foods?query=skyr&language=da",
        "/api/nutrition/foods/skyr",
        "/api/nutrition/foods/0",
    ] {
        let (status, body) = get(&app, uri).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{uri}: {body}");
        assert_eq!(body["status"], 400, "{uri}: {body}");
    }
    let (status, body) = get(&app, "/api/nutrition/foods/424242").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"], "Food 424242 was not found");
    let (status, _) = send_request(app, "/api/nutrition/foods?query=skyr", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn a_recorded_pin_skips_the_import_and_a_rebuilt_database_reuses_the_cached_workbook() {
    let server = MockServer::start().await;
    let bytes = first_dataset();
    serve(&server, "6.1", &bytes, 1).await;
    let dir = TempDir::new().unwrap();
    let pin = pin(&server, "6.1", sha256(&bytes));
    let _ = imported(&dir, pin.clone()).await;

    let (app, import) = open(&dir, "food.db", pin.clone()).await;
    assert!(import.is_none(), "a recorded pin needs no import");
    let (status, body) = get(&app, "/api/nutrition/foods?query=skyr").await;
    assert_eq!(status, StatusCode::OK, "{body}");

    // A fresh food.db re-imports from the verified cached workbook without downloading again.
    let (app, import) = open(&dir, "rebuilt-food.db", pin).await;
    import.expect("an empty food.db records no pin").run().await;
    let (status, body) = get(&app, "/api/nutrition/foods/1693").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    server.verify().await;
}

#[tokio::test]
async fn without_a_previous_import_a_failed_import_reports_failed_without_upstream_detail() {
    let server = MockServer::start().await;
    let bytes = first_dataset();
    serve(&server, "mismatch", &bytes, 1).await;
    let changed_unit = workbook(&parameters("mg/100g"), &[skyr(11.0)]);
    serve(&server, "unit", &changed_unit, 1).await;
    Mock::given(method("GET"))
        .and(path("/files/broken.xlsx"))
        .respond_with(
            ResponseTemplate::new(500).set_body_string("synthetic upstream failure detail"),
        )
        .expect(1)
        .mount(&server)
        .await;

    for pin in [
        pin(&server, "mismatch", sha256(b"a different workbook")),
        pin(&server, "unit", sha256(&changed_unit)),
        pin(&server, "broken", sha256(&bytes)),
    ] {
        let dir = TempDir::new().unwrap();
        let version = pin.version.clone();
        let app = imported(&dir, pin).await;
        for uri in [
            "/api/nutrition/foods?query=skyr",
            "/api/nutrition/foods/1693",
        ] {
            let (status, body) = get(&app, uri).await;
            assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{version} {uri}");
            assert_unavailable(&body, "failed");
            let text = body.to_string();
            assert!(
                !text.contains("synthetic upstream") && !text.contains("500"),
                "{text}"
            );
        }
        // Only a verified workbook is cached.
        let expected: &[&str] = if version == "unit" {
            &["dtu-fcdb-unit.xlsx"]
        } else {
            &[]
        };
        assert_eq!(cached_files(&dir), expected, "{version}");
    }
}

#[tokio::test]
async fn a_changed_pin_reimports_while_failed_attempts_keep_the_previous_import() {
    let server = MockServer::start().await;
    let first = first_dataset();
    let second = second_dataset();
    serve(&server, "1", &first, 1).await;
    serve(&server, "2", &second, 2).await;
    Mock::given(method("GET"))
        .and(path("/files/2-unreachable.xlsx"))
        .respond_with(ResponseTemplate::new(503).set_body_string("synthetic outage notice"))
        .mount(&server)
        .await;
    let dir = TempDir::new().unwrap();
    let _ = imported(&dir, pin(&server, "1", sha256(&first))).await;

    let unreachable = DatasetPin {
        url: format!("{}/files/2-unreachable.xlsx", server.uri())
            .parse()
            .unwrap(),
        ..pin(&server, "2", sha256(&second))
    };
    for failing in [
        unreachable,
        pin(&server, "2", sha256(b"not the second workbook")),
    ] {
        let (app, import) = open(&dir, "food.db", failing).await;
        let (status, body) = get(&app, "/api/nutrition/foods/3").await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_unavailable(&body, "importing");
        import.unwrap().run().await;
        let (status, banana) = get(&app, "/api/nutrition/foods/3").await;
        assert_eq!(status, StatusCode::OK, "{banana}");
        assert_eq!(banana["source"]["version"], "1");
        assert!(!banana.to_string().contains("synthetic outage"));
    }

    let (app, import) = open(&dir, "food.db", pin(&server, "2", sha256(&second))).await;
    import.expect("a changed pin re-imports").run().await;
    let (status, skyr) = get(&app, "/api/nutrition/foods/1693").await;
    assert_eq!(status, StatusCode::OK, "{skyr}");
    assert_eq!(skyr["source"]["version"], "2");
    assert_eq!(parameter_value(&skyr, "protein")["value"], 12.0);
    let (status, _) = get(&app, "/api/nutrition/foods/3").await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "a Food dropped by the new version"
    );
    let (_, body) = get(&app, "/api/nutrition/foods?query=rugbr%C3%B8d").await;
    assert_eq!(body["foods"][0]["food_id"], 5, "{body}");
    assert_eq!(cached_files(&dir), ["dtu-fcdb-2.xlsx"]);
    server.verify().await;
}
