//! Meal Logs and the Nutrition Day through the nutrition routes, over a real `app.db` and a real
//! `food.db` imported from a synthetic DTU-shaped workbook served by `wiremock`.

mod common;

use axum::{
    Router,
    body::Body,
    http::{Method, Request, StatusCode, header},
};
use common::food_workbook::{first_dataset, second_dataset, sha256};
use common::{send_request_with_method, test_app_with_db};
use openhome_api::{
    AppState,
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
use tower::ServiceExt;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};

const API_KEY: &str = "test-api-key";

/// The nutrition routes over one `app.db` and a `food.db` in a temporary directory.
struct Nutrition {
    server: MockServer,
    dir: TempDir,
    state: AppState,
    app: Router,
}

impl Nutrition {
    /// Imports the first synthetic dataset as version "1".
    async fn start() -> Self {
        let server = MockServer::start().await;
        let dir = TempDir::new().unwrap();
        let (_, state) = test_app_with_db().await;
        let app = Self::import(&server, &dir, &state, "1", first_dataset()).await;
        Self {
            server,
            dir,
            state,
            app,
        }
    }

    /// Imports `bytes` as `version` into the same `food.db`, as a pin change on restart does.
    async fn import(
        server: &MockServer,
        dir: &TempDir,
        state: &AppState,
        version: &str,
        bytes: Vec<u8>,
    ) -> Router {
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
        app(foods, photos(dir), state.clone())
    }

    async fn reimport(&mut self, version: &str, bytes: Vec<u8>) {
        self.app = Self::import(&self.server, &self.dir, &self.state, version, bytes).await;
    }

    async fn send(&self, method: Method, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
        send_request_with_method(self.app.clone(), uri, method, body, Some(API_KEY)).await
    }

    async fn get(&self, uri: &str) -> (StatusCode, Value) {
        self.send(Method::GET, uri, None).await
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

    async fn day(&self, day: &str) -> Vec<Value> {
        let (status, body) = self
            .get(&format!("/api/nutrition/meal-logs?day={day}"))
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        body["meal_logs"].as_array().unwrap().clone()
    }

    /// GETs a media route: its status, content type and bytes.
    async fn media(&self, uri: &str) -> (StatusCode, Option<String>, Vec<u8>) {
        let request = Request::get(uri)
            .header(header::AUTHORIZATION, format!("Bearer {API_KEY}"))
            .body(Body::empty())
            .unwrap();
        let response = self.app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .map(|value| value.to_str().unwrap().to_owned());
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        (status, content_type, bytes.to_vec())
    }

    /// PUTs `bytes` as the multipart file field `field` of a Meal Log's photo.
    async fn upload(&self, id: &str, field: &str, bytes: &[u8]) -> (StatusCode, Value) {
        let mut body = format!(
            "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"note\"\r\n\r\nignored\r\n\
             --{BOUNDARY}\r\nContent-Disposition: form-data; name=\"{field}\"; \
             filename=\"meal\"\r\nContent-Type: application/octet-stream\r\n\r\n"
        )
        .into_bytes();
        body.extend_from_slice(bytes);
        body.extend_from_slice(format!("\r\n--{BOUNDARY}--\r\n").as_bytes());
        let request = Request::put(format!("/api/nutrition/meal-logs/{id}/photo"))
            .header(header::AUTHORIZATION, format!("Bearer {API_KEY}"))
            .header(
                header::CONTENT_TYPE,
                format!("multipart/form-data; boundary={BOUNDARY}"),
            )
            .body(Body::from(body))
            .unwrap();
        let response = self.app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        (status, serde_json::from_slice(&bytes).unwrap())
    }

    /// Every file in the photo directory, sorted.
    fn photo_files(&self) -> Vec<String> {
        let Ok(entries) = std::fs::read_dir(self.dir.path().join("meal-photos")) else {
            return Vec::new();
        };
        let mut files: Vec<String> = entries
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        files.sort();
        files
    }
}

const BOUNDARY: &str = "openhome-test-boundary";

/// Synthetic photos: no real photo is ever committed.
mod synthetic {
    use base64::Engine;
    use image::{ImageEncoder, Rgb, RgbImage, codecs::jpeg::JpegEncoder, codecs::png::PngEncoder};

    /// The camera make written into the EXIF block, to look for in stored files.
    pub const CAMERA: &[u8] = b"OpenHomeTestCam";

    /// A frame whose left half is red and right half blue, as the sensor recorded it.
    fn frame(width: u32, height: u32) -> RgbImage {
        RgbImage::from_fn(width, height, |x, y| {
            let shade = (y * 255 / height) as u8;
            if x < width / 2 {
                Rgb([230, shade / 4, 30])
            } else {
                Rgb([30, shade / 4, 230])
            }
        })
    }

    /// A JPEG of `frame` carrying EXIF with `orientation`, a camera make and a GPS latitude.
    pub fn jpeg_with_gps(width: u32, height: u32, orientation: u16) -> Vec<u8> {
        let mut jpeg = Vec::new();
        JpegEncoder::new_with_quality(&mut jpeg, 85)
            .write_image(
                frame(width, height).as_raw(),
                width,
                height,
                image::ExtendedColorType::Rgb8,
            )
            .unwrap();
        // A little-endian TIFF: IFD0 (Make, Orientation, GPS pointer), its Make string, then a
        // GPS IFD with GPSLatitudeRef "N" and GPSLatitude 55 deg 40' 30".
        let mut tiff = b"II\x2a\x00\x08\x00\x00\x00".to_vec();
        let entry = |tiff: &mut Vec<u8>, tag: u16, kind: u16, count: u32, value: u32| {
            tiff.extend_from_slice(&tag.to_le_bytes());
            tiff.extend_from_slice(&kind.to_le_bytes());
            tiff.extend_from_slice(&count.to_le_bytes());
            tiff.extend_from_slice(&value.to_le_bytes());
        };
        let make_at = 8 + 2 + 3 * 12 + 4;
        let gps_at = make_at + CAMERA.len() as u32 + 1;
        tiff.extend_from_slice(&3_u16.to_le_bytes());
        entry(&mut tiff, 0x010F, 2, CAMERA.len() as u32 + 1, make_at);
        entry(&mut tiff, 0x0112, 3, 1, u32::from(orientation));
        entry(&mut tiff, 0x8825, 4, 1, gps_at);
        tiff.extend_from_slice(&0_u32.to_le_bytes());
        tiff.extend_from_slice(CAMERA);
        tiff.push(0);
        let latitude_at = gps_at + 2 + 2 * 12 + 4;
        tiff.extend_from_slice(&2_u16.to_le_bytes());
        entry(&mut tiff, 0x0001, 2, 2, u32::from(b'N'));
        entry(&mut tiff, 0x0002, 5, 3, latitude_at);
        tiff.extend_from_slice(&0_u32.to_le_bytes());
        for (numerator, denominator) in [(55_u32, 1_u32), (40, 1), (3000, 100)] {
            tiff.extend_from_slice(&numerator.to_le_bytes());
            tiff.extend_from_slice(&denominator.to_le_bytes());
        }
        let mut app1 = vec![0xFF, 0xE1];
        app1.extend_from_slice(&(2 + 6 + tiff.len() as u16).to_be_bytes());
        app1.extend_from_slice(b"Exif\0\0");
        app1.extend_from_slice(&tiff);
        jpeg.splice(2..2, app1);
        jpeg
    }

    pub fn png(width: u32, height: u32) -> Vec<u8> {
        let mut png = Vec::new();
        PngEncoder::new(&mut png)
            .write_image(
                frame(width, height).as_raw(),
                width,
                height,
                image::ExtendedColorType::Rgb8,
            )
            .unwrap();
        png
    }

    pub fn base64(bytes: &[u8]) -> String {
        base64::engine::general_purpose::STANDARD.encode(bytes)
    }

    pub fn contains(haystack: &[u8], needle: &[u8]) -> bool {
        haystack
            .windows(needle.len())
            .any(|window| window == needle)
    }

    /// Decodes a stored AVIF: its size and the RGBA pixel at (`x`, `y`).
    pub fn avif_pixel(avif: &[u8], x: u32, y: u32) -> ((u32, u32), (u8, u8, u8)) {
        assert!(libavif::is_avif(avif), "not an AVIF file");
        let pixels = libavif::decode_rgb(avif).unwrap();
        let (red, green, blue, _) = pixels.pixel(x, y);
        ((pixels.width(), pixels.height()), (red, green, blue))
    }
}

async fn open(
    dir: &TempDir,
    pin: DatasetPin,
) -> (
    FoodDatabase,
    Option<openhome_api::services::nutrition::food_database::FoodImport>,
) {
    let url = format!("sqlite:{}", dir.path().join("food.db").display());
    let pool = db::connect_sqlite(&url).await.unwrap();
    FoodDatabase::open(pool, pin, dir.path().join("food-dataset"))
        .await
        .unwrap()
}

fn photos(dir: &TempDir) -> MealPhotos {
    MealPhotos::new(dir.path().join("meal-photos"))
}

fn app(foods: FoodDatabase, photos: MealPhotos, state: AppState) -> Router {
    // Product Portions are covered in nutrition_products; nothing here reads a barcode.
    let labels = OpenFoodFacts::new("http://127.0.0.1:9/".parse().unwrap()).unwrap();
    routes::nutrition::router(foods, photos, labels)
        .with_state(state)
        .layer(axum::middleware::from_fn(|request, next| {
            auth_middleware(request, next, ApiKey::new(API_KEY.into()))
        }))
}

fn breakfast(request_id: &str) -> Value {
    json!({
        "eaten_at": "2026-10-29T08:15:00+01:00",
        "note": "  after the run ",
        "portions": [{"food_id": 1693, "grams": 150}, {"food_id": 907, "grams": 120}],
        "request_id": request_id,
    })
}

fn at(eaten_at: &str, request_id: &str) -> Value {
    json!({
        "eaten_at": eaten_at,
        "portions": [{"food_id": 1693, "grams": 100}],
        "request_id": request_id,
    })
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
async fn a_logged_meal_keeps_scaled_snapshots_with_sources_and_totals() {
    let api = Nutrition::start().await;

    let meal_log = api.logged(breakfast("breakfast-1")).await;

    assert_eq!(meal_log["eaten_at"], "2026-10-29T08:15:00+01:00");
    assert_eq!(meal_log["day"], "2026-10-29");
    assert_eq!(meal_log["note"], "after the run");
    let skyr = &meal_log["portions"][0];
    assert_eq!(
        (
            &skyr["food_id"],
            &skyr["name_en"],
            &skyr["grams"],
            &skyr["dataset_version"]
        ),
        (
            &json!(1693),
            &json!("Skyr, 0.2 % fat"),
            &json!(150.0),
            &json!("1")
        )
    );
    assert_eq!(
        skyr["nutrients"],
        json!({
            "energy_kcal": {"value": 93.0, "unit": "kcal", "source": "food"},
            "protein": {"value": 16.5, "unit": "g", "source": "food"},
            "available_carbohydrate": {"value": 6.0, "unit": "g", "source": "food"},
            "fat": {"value": 0.3, "unit": "g", "source": "food"},
            "dietary_fibre": {"value": 0.0, "unit": "g", "source": "food"},
            "alcohol": {"value": 0.0, "unit": "g", "source": "food"},
        })
    );
    // The chicken has no carbohydrate, fibre or alcohol value: unknown, never zero.
    assert_eq!(
        meal_log["portions"][1]["nutrients"]["available_carbohydrate"],
        json!({"value": null, "unit": "g", "source": null})
    );
    assert_eq!(
        meal_log["totals"],
        json!({
            "energy_kcal": {"value": 273.0, "unit": "kcal", "unknown_portions": 0, "food": 273.0, "supplements": null},
            "protein": {"value": 41.7, "unit": "g", "unknown_portions": 0, "food": 41.7, "supplements": null},
            "available_carbohydrate": {"value": 6.0, "unit": "g", "unknown_portions": 1, "food": 6.0, "supplements": null},
            "fat": {"value": 8.7, "unit": "g", "unknown_portions": 0, "food": 8.7, "supplements": null},
            "dietary_fibre": {"value": 0.0, "unit": "g", "unknown_portions": 1, "food": 0.0, "supplements": null},
            "alcohol": {"value": 0.0, "unit": "g", "unknown_portions": 1, "food": 0.0, "supplements": null},
        })
    );
    assert_eq!(api.day("2026-10-29").await, [meal_log]);
}

#[tokio::test]
async fn nutrition_day_totals_every_parameter_with_units_and_credits_the_dataset() {
    let api = Nutrition::start().await;
    api.logged(breakfast("breakfast-1")).await;
    let lunch = api
        .logged(json!({
            "eaten_at": "2026-10-29T12:30:00+01:00",
            "portions": [{"food_id": 3, "grams": 100}],
            "request_id": "lunch-1",
        }))
        .await;

    let (status, day) = api.get("/api/nutrition/days/2026-10-29").await;

    assert_eq!(status, StatusCode::OK, "{day}");
    assert_eq!(
        (
            &day["day"],
            &day["time_zone"],
            &day["day_start"],
            &day["day_end"]
        ),
        (
            &json!("2026-10-29"),
            &json!("Europe/Copenhagen"),
            &json!("2026-10-29T00:00:00+01:00"),
            &json!("2026-10-30T00:00:00+01:00")
        )
    );
    assert_eq!(
        day["totals"]["energy_kcal"],
        json!({"value": 363.0, "unit": "kcal", "unknown_portions": 0, "food": 363.0, "supplements": null})
    );
    assert_eq!(
        day["totals"]["available_carbohydrate"],
        json!({"value": 26.0, "unit": "g", "unknown_portions": 1, "food": 26.0, "supplements": null})
    );
    // Every per-100 g parameter, in database order; waste, density and pH describe the Food.
    let keys: Vec<&str> = day["nutrients"]
        .as_array()
        .unwrap()
        .iter()
        .map(|nutrient| nutrient["key"].as_str().unwrap())
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
            "epa"
        ]
    );
    assert_eq!(
        nutrient(&day, "vitamin_d"),
        &json!({
            "key": "vitamin_d", "name_da": "D-vitamin", "name_en": "Vitamin D",
            "group": {"name_da": "Vitaminer", "name_en": "Vitamins"},
            "unit": "µg", "value": 0.0, "unknown_portions": 2, "food": 0.0, "supplements": null,
        })
    );
    assert_eq!(nutrient(&day, "epa")["value"], 0.012);
    assert_eq!(
        (
            &nutrient(&day, "glycerol")["value"],
            &nutrient(&day, "glycerol")["unknown_portions"]
        ),
        (&Value::Null, &json!(3))
    );
    assert_eq!(day["meal_logs"].as_array().unwrap().len(), 2);
    assert_eq!(day["meal_logs"][1], lunch);
    assert_eq!(
        day["source"]["credit"],
        "© The Danish Food Composition Database, version 1"
    );

    let (_, empty) = api.get("/api/nutrition/days/2026-10-28").await;
    assert_eq!(empty["meal_logs"], json!([]));
    assert_eq!(
        empty["totals"]["energy_kcal"],
        json!({"value": null, "unit": "kcal", "unknown_portions": 0, "food": null, "supplements": null})
    );
    let (status, _) = api.get("/api/nutrition/days/2026-10-32").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn a_reimported_food_database_never_changes_past_meal_logs() {
    let mut api = Nutrition::start().await;
    let breakfast = api.logged(breakfast("breakfast-1")).await;
    let banana = api
        .logged(json!({
            "eaten_at": "2026-10-29T15:00:00+01:00",
            "portions": [{"food_id": 3, "grams": 120}],
            "request_id": "banana-1",
        }))
        .await;
    let (_, day_before) = api.get("/api/nutrition/days/2026-10-29").await;

    // Version 2 changes Skyr's protein and drops the Banana.
    api.reimport("2", second_dataset()).await;

    assert_eq!(api.day("2026-10-29").await, [breakfast, banana]);
    let (_, day_after) = api.get("/api/nutrition/days/2026-10-29").await;
    assert_eq!(day_after["totals"], day_before["totals"]);
    assert_eq!(day_after["nutrients"], day_before["nutrients"]);
    let later = api
        .logged(json!({
            "eaten_at": "2026-10-29T20:00:00+01:00",
            "portions": [{"food_id": 1693, "grams": 150}],
            "request_id": "breakfast-2",
        }))
        .await;
    assert_eq!(later["portions"][0]["nutrients"]["protein"]["value"], 18.0);
    assert_eq!(later["portions"][0]["dataset_version"], "2");
}

#[tokio::test]
async fn a_retried_request_id_replays_conflicts_on_changes_and_never_recreates_a_deleted_log() {
    let api = Nutrition::start().await;
    let meal_log = api.logged(breakfast("breakfast-1")).await;

    // The same instant written in UTC with the same note after trimming is the same request.
    let mut retry = breakfast("breakfast-1");
    retry["eaten_at"] = json!("2026-10-29T07:15:00Z");
    retry["note"] = json!("after the run");
    let (status, replayed) = api.log(retry).await;
    assert_eq!(status, StatusCode::OK, "{replayed}");
    assert_eq!(replayed, json!({"meal_log": meal_log, "replayed": true}));

    let mut changed = breakfast("breakfast-1");
    changed["portions"][0]["grams"] = json!(200);
    let (status, body) = api.log(changed).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(api.day("2026-10-29").await.len(), 1);

    let id = meal_log["id"].as_str().unwrap().to_owned();
    let (status, _) = api
        .send(
            Method::DELETE,
            &format!("/api/nutrition/meal-logs/{id}"),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, body) = api.log(breakfast("breakfast-1")).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert!(
        body["error"].as_str().unwrap().contains("deleted"),
        "{body}"
    );
    assert_eq!(api.day("2026-10-29").await, Vec::<Value>::new());

    // A rejected request consumes no request_id.
    let mut unknown = at("2026-10-29T09:00:00+01:00", "rejected-first");
    unknown["portions"][0]["food_id"] = json!(424_242);
    let (status, body) = api.log(unknown).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let (status, _) = api
        .log(at("2026-10-29T09:00:00+01:00", "rejected-first"))
        .await;
    assert_eq!(status, StatusCode::CREATED);
}

#[tokio::test]
async fn meal_logs_belong_to_the_copenhagen_day_of_their_time_across_midnight_and_daylight_saving()
{
    let api = Nutrition::start().await;
    // (eaten_at, Copenhagen time it is shown in, Copenhagen day)
    let cases = [
        // Midnight in winter time (UTC+1).
        (
            "2026-10-29T23:59:59+01:00",
            "2026-10-29T23:59:59+01:00",
            "2026-10-29",
        ),
        (
            "2026-10-29T23:30:00Z",
            "2026-10-30T00:30:00+01:00",
            "2026-10-30",
        ),
        // Summer time ends on 2026-10-25: that day lasts 25 hours.
        (
            "2026-10-24T22:00:00Z",
            "2026-10-25T00:00:00+02:00",
            "2026-10-25",
        ),
        (
            "2026-10-25T00:30:00Z",
            "2026-10-25T02:30:00+02:00",
            "2026-10-25",
        ),
        (
            "2026-10-25T01:30:00Z",
            "2026-10-25T02:30:00+01:00",
            "2026-10-25",
        ),
        (
            "2026-10-25T22:59:59Z",
            "2026-10-25T23:59:59+01:00",
            "2026-10-25",
        ),
        // Summer time starts on 2026-03-29: that day lasts 23 hours.
        (
            "2026-03-28T23:00:00Z",
            "2026-03-29T00:00:00+01:00",
            "2026-03-29",
        ),
        (
            "2026-03-29T21:59:59Z",
            "2026-03-29T23:59:59+02:00",
            "2026-03-29",
        ),
        (
            "2026-03-29T22:00:00Z",
            "2026-03-30T00:00:00+02:00",
            "2026-03-30",
        ),
    ];
    for (index, (eaten_at, shown, day)) in cases.into_iter().enumerate() {
        let meal_log = api.logged(at(eaten_at, &format!("day-{index}"))).await;
        assert_eq!(
            (&meal_log["eaten_at"], &meal_log["day"]),
            (&json!(shown), &json!(day)),
            "{eaten_at}"
        );
    }

    let times = |logs: Vec<Value>| -> Vec<String> {
        logs.iter()
            .map(|log| log["eaten_at"].as_str().unwrap().to_owned())
            .collect()
    };
    assert_eq!(
        times(api.day("2026-10-25").await),
        [
            "2026-10-25T00:00:00+02:00",
            "2026-10-25T02:30:00+02:00",
            "2026-10-25T02:30:00+01:00",
            "2026-10-25T23:59:59+01:00"
        ]
    );
    let (_, day) = api.get("/api/nutrition/days/2026-03-29").await;
    assert_eq!(
        (&day["day_start"], &day["day_end"]),
        (
            &json!("2026-03-29T00:00:00+01:00"),
            &json!("2026-03-30T00:00:00+02:00")
        )
    );
    assert_eq!(day["meal_logs"].as_array().unwrap().len(), 2);

    let (status, range) = api
        .get("/api/nutrition/meal-logs?from=2026-10-25&to=2026-10-30")
        .await;
    assert_eq!(status, StatusCode::OK, "{range}");
    assert_eq!(
        (&range["from"], &range["to"], &range["time_zone"]),
        (
            &json!("2026-10-25"),
            &json!("2026-10-30"),
            &json!("Europe/Copenhagen")
        )
    );
    assert_eq!(range["meal_logs"].as_array().unwrap().len(), 6);
}

#[tokio::test]
async fn backfilled_times_are_accepted_and_future_or_offsetless_times_are_refused() {
    let api = Nutrition::start().await;
    let a_year_ago = chrono::Utc::now() - chrono::TimeDelta::days(365);
    let backfilled = api
        .logged(at(&a_year_ago.to_rfc3339(), "forgotten-dinner"))
        .await;
    let day = openhome_api::services::training::CalendarDay::of(a_year_ago).to_string();
    assert_eq!(backfilled["day"], day);
    assert_eq!(api.day(&day).await, [backfilled]);

    let tomorrow = chrono::Utc::now() + chrono::TimeDelta::days(1);
    for (eaten_at, request_id) in [
        (tomorrow.to_rfc3339(), "tomorrow"),
        ("2026-10-29T08:15:00".to_owned(), "no-offset"),
        ("yesterday morning".to_owned(), "words"),
    ] {
        let (status, body) = api.log(at(&eaten_at, request_id)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{eaten_at}: {body}");
        assert!(
            body["error"].as_str().unwrap().contains("eaten_at"),
            "{body}"
        );
    }
}

#[tokio::test]
async fn invalid_requests_are_rejected_without_writing() {
    let api = Nutrition::start().await;
    let valid = at("2026-10-29T08:00:00+01:00", "invalid-1");
    let with = |field: &str, value: Value| {
        let mut body = valid.clone();
        body[field] = value;
        body
    };
    for body in [
        with("portions", json!([])),
        with("portions", json!([{"food_id": 1693, "grams": 0}])),
        with("portions", json!([{"food_id": 1693, "grams": -5}])),
        with("portions", json!([{"food_id": 1693, "grams": 5001}])),
        with("portions", json!([{"food_id": 0, "grams": 100}])),
        with("portions", json!([{"food_id": 1693}])),
        with(
            "portions",
            json!([{"food_id": 1693, "grams": 100, "kcal": 5}]),
        ),
        with("request_id", json!(" ")),
        with("note", json!("x".repeat(501))),
        with("calories", json!(500)),
    ] {
        let (status, response) = api.log(body.clone()).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}: {response}");
        assert_eq!(response["status"], 400, "{response}");
    }
    for uri in [
        "/api/nutrition/meal-logs",
        "/api/nutrition/meal-logs?day=29-10-2026",
        "/api/nutrition/meal-logs?day=2026-10-29&from=2026-10-29&to=2026-10-29",
        "/api/nutrition/meal-logs?from=2026-10-29",
        "/api/nutrition/meal-logs?from=2026-10-29&to=2026-10-28",
        "/api/nutrition/meal-logs?from=2026-01-01&to=2026-10-29",
        "/api/nutrition/meal-logs?day=2026-10-29&owner=me",
    ] {
        let (status, response) = api.get(uri).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{uri}: {response}");
    }
    assert_eq!(api.day("2026-10-29").await, Vec::<Value>::new());
    let (status, _) = send_request_with_method(
        api.app.clone(),
        "/api/nutrition/meal-logs?day=2026-10-29",
        Method::GET,
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn an_update_changes_time_note_or_portions_and_a_delete_removes_the_log() {
    let mut api = Nutrition::start().await;
    let meal_log = api.logged(breakfast("breakfast-1")).await;
    let id = meal_log["id"].as_str().unwrap().to_owned();
    let uri = format!("/api/nutrition/meal-logs/{id}");

    // Moving it to the evening before moves it to that day; omitted fields stay.
    let (status, moved) = api
        .send(
            Method::PATCH,
            &uri,
            Some(json!({"eaten_at": "2026-10-28T21:00:00+01:00"})),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{moved}");
    assert_eq!(
        (&moved["day"], &moved["note"], &moved["portions"]),
        (
            &json!("2026-10-28"),
            &meal_log["note"],
            &meal_log["portions"]
        )
    );
    assert_eq!(api.day("2026-10-29").await, Vec::<Value>::new());
    assert_eq!(api.day("2026-10-28").await, std::slice::from_ref(&moved));

    // Replacing Portions snapshots the current database; null clears the note.
    api.reimport("2", second_dataset()).await;
    let (status, corrected) = api
        .send(
            Method::PATCH,
            &uri,
            Some(json!({"note": null, "portions": [{"food_id": 1693, "grams": 200}]})),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{corrected}");
    assert_eq!(corrected["note"], Value::Null);
    assert_eq!(corrected["eaten_at"], moved["eaten_at"]);
    assert_eq!(corrected["portions"].as_array().unwrap().len(), 1);
    assert_eq!(
        corrected["portions"][0]["nutrients"]["protein"]["value"],
        24.0
    );
    assert_eq!(corrected["totals"]["energy_kcal"]["value"], 124.0);

    for body in [
        json!({"eaten_at": null}),
        json!({"portions": null}),
        json!({"portions": []}),
        json!({"portions": [{"food_id": 1693, "grams": 0}]}),
        json!({"eaten_at": "2026-10-28 21:00"}),
        json!({"request_id": "x"}),
    ] {
        let (status, response) = api.send(Method::PATCH, &uri, Some(body.clone())).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}: {response}");
    }
    let (status, unknown) = api
        .send(
            Method::PATCH,
            &uri,
            Some(json!({"portions": [{"food_id": 3, "grams": 100}]})),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "a dropped Food: {unknown}");
    assert_eq!(api.day("2026-10-28").await, [corrected]);

    let (status, _) = api.send(Method::DELETE, &uri, None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(api.day("2026-10-28").await, Vec::<Value>::new());
    for method in [Method::DELETE, Method::PATCH] {
        let (status, body) = api
            .send(method.clone(), &uri, Some(json!({"note": "late"})))
            .await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{method}: {body}");
    }
}

#[tokio::test]
async fn while_the_food_database_is_unavailable_logs_stay_readable_and_editable() {
    let api = Nutrition::start().await;
    let meal_log = api.logged(breakfast("breakfast-1")).await;
    let id = meal_log["id"].as_str().unwrap().to_owned();
    // The next start re-imports a new pin; until it completes, Food lookups are unavailable.
    let pin = DatasetPin {
        version: "2".into(),
        url: format!("{}/files/never.xlsx", api.server.uri())
            .parse()
            .unwrap(),
        sha256: sha256(b"never served"),
    };
    let (foods, import) = open(&api.dir, pin).await;
    assert!(import.is_some());
    let importing = Nutrition {
        app: app(foods, photos(&api.dir), api.state.clone()),
        ..api
    };

    for (status, body) in [
        importing
            .log(at("2026-10-29T12:00:00+01:00", "lunch-1"))
            .await,
        importing.get("/api/nutrition/days/2026-10-29").await,
        importing
            .send(
                Method::PATCH,
                &format!("/api/nutrition/meal-logs/{id}"),
                Some(json!({"portions": [{"food_id": 1693, "grams": 10}]})),
            )
            .await,
    ] {
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
        assert_eq!(
            (&body["code"], &body["state"]),
            (&json!("food_database_unavailable"), &json!("importing"))
        );
    }
    assert_eq!(importing.day("2026-10-29").await, [meal_log]);
    let (status, edited) = importing
        .send(
            Method::PATCH,
            &format!("/api/nutrition/meal-logs/{id}"),
            Some(json!({"note": "rough estimate"})),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{edited}");
    assert_eq!(edited["note"], "rough estimate");
}

fn with_photo(mut body: Value, photo: &[u8]) -> Value {
    body["photo"] = json!(synthetic::base64(photo));
    body
}

#[tokio::test]
async fn a_photo_is_stored_upright_without_metadata_as_avif_with_a_thumbnail() {
    let api = Nutrition::start().await;
    // A 2400x1200 sensor frame the camera held a quarter turn (EXIF orientation 6), with a GPS
    // position and a camera make.
    let jpeg = synthetic::jpeg_with_gps(2400, 1200, 6);
    assert!(synthetic::contains(&jpeg, synthetic::CAMERA));

    let meal_log = api.logged(with_photo(breakfast("photo-1"), &jpeg)).await;

    let photo = &meal_log["photo"];
    let id = photo["id"].as_str().unwrap();
    assert_eq!(
        *photo,
        json!({
            "id": id,
            "width": 800,
            "height": 1600,
            "url": format!("/api/nutrition/meal-photos/{id}"),
            "thumbnail": {
                "width": 200,
                "height": 400,
                "url": format!("/api/nutrition/meal-photos/{id}/thumbnail"),
            },
        })
    );
    for (url, size, top, bottom) in [
        (&photo["url"], (800, 1600), (400, 200), (400, 1400)),
        (
            &photo["thumbnail"]["url"],
            (200, 400),
            (100, 50),
            (100, 350),
        ),
    ] {
        let (status, content_type, avif) = api.media(url.as_str().unwrap()).await;
        assert_eq!(
            (status, content_type.as_deref()),
            (StatusCode::OK, Some("image/avif"))
        );
        // Re-encoded from pixels: no EXIF block, camera make or GPS position survives.
        assert!(!synthetic::contains(&avif, b"Exif"), "{url}");
        assert!(!synthetic::contains(&avif, synthetic::CAMERA), "{url}");
        // Upright: the sensor's red left half is now the top, its blue right half the bottom.
        let (decoded, (red, _, blue)) = synthetic::avif_pixel(&avif, top.0, top.1);
        assert_eq!(decoded, size, "{url}");
        assert!(red > 180 && blue < 80, "top of {url} is ({red}, {blue})");
        let (_, (red, _, blue)) = synthetic::avif_pixel(&avif, bottom.0, bottom.1);
        assert!(blue > 180 && red < 80, "bottom of {url} is ({red}, {blue})");
    }
    // Only the AVIF photo and thumbnail are kept, never the uploaded JPEG.
    assert_eq!(
        api.photo_files(),
        [format!("{id}.avif"), format!("{id}.thumbnail.avif")]
    );
    assert_eq!(api.day("2026-10-29").await, std::slice::from_ref(&meal_log));
    let (status, nutrition_day) = api.get("/api/nutrition/days/2026-10-29").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(nutrition_day["meal_logs"], json!([meal_log]));
}

#[tokio::test]
async fn photos_are_scaled_down_to_their_long_side_and_never_up() {
    let api = Nutrition::start().await;
    for (index, (width, height, photo, thumbnail)) in [
        (3200, 800, (1600, 400), (400, 100)),
        (1000, 800, (1000, 800), (400, 320)),
        (300, 200, (300, 200), (300, 200)),
    ]
    .into_iter()
    .enumerate()
    {
        let mut body = at("2026-10-29T12:00:00+01:00", &format!("scaled-{index}"));
        // A data URL, as some agents send it, is accepted too.
        body["photo"] = json!(format!(
            "data:image/png;base64,{}",
            synthetic::base64(&synthetic::png(width, height))
        ));
        let logged = api.logged(body).await["photo"].clone();
        assert_eq!(
            (
                (&logged["width"], &logged["height"]),
                (
                    &logged["thumbnail"]["width"],
                    &logged["thumbnail"]["height"]
                )
            ),
            (
                (&json!(photo.0), &json!(photo.1)),
                (&json!(thumbnail.0), &json!(thumbnail.1))
            ),
            "{width}x{height}"
        );
        let (_, _, avif) = api.media(logged["url"].as_str().unwrap()).await;
        assert_eq!(synthetic::avif_pixel(&avif, 0, 0).0, photo);
    }
}

#[tokio::test]
async fn an_undecodable_or_oversized_photo_rejects_the_whole_request_and_writes_nothing() {
    let api = Nutrition::start().await;
    let existing = api
        .logged(at("2026-10-28T12:00:00+01:00", "existing"))
        .await;
    let existing_uri = format!(
        "/api/nutrition/meal-logs/{}",
        existing["id"].as_str().unwrap()
    );
    let jpeg_start = |length: usize| {
        let mut bytes = vec![0xFF, 0xD8, 0xFF, 0xE0];
        bytes.resize(length, 0x42);
        bytes
    };
    let ten_mb = 10 * 1024 * 1024;
    let cases: [(&str, Value, StatusCode); 7] = [
        ("not base64", json!("not base64!"), StatusCode::BAD_REQUEST),
        (
            "a GIF",
            json!(synthetic::base64(b"GIF89a\x01\x00\x01\x00")),
            StatusCode::BAD_REQUEST,
        ),
        (
            "a broken JPEG",
            json!(synthetic::base64(&jpeg_start(2_000))),
            StatusCode::BAD_REQUEST,
        ),
        ("not a string", json!(42), StatusCode::BAD_REQUEST),
        (
            "wider than 12000 px",
            json!(synthetic::base64(&synthetic::png(12_001, 1))),
            StatusCode::PAYLOAD_TOO_LARGE,
        ),
        (
            "over 10 MB, inside the body limit",
            json!(synthetic::base64(&jpeg_start(ten_mb + 1))),
            StatusCode::PAYLOAD_TOO_LARGE,
        ),
        (
            "over the 15 MB body limit",
            json!(synthetic::base64(&jpeg_start(12 * 1024 * 1024))),
            StatusCode::PAYLOAD_TOO_LARGE,
        ),
    ];
    for (case, photo, expected) in cases {
        let mut body = at("2026-10-29T12:00:00+01:00", "rejected");
        body["photo"] = photo.clone();
        let (status, response) = api.log(body).await;
        assert_eq!(status, expected, "log_meal with {case}: {response}");
        assert_eq!(response["status"], expected.as_u16(), "{case}: {response}");

        let (status, response) = api
            .send(
                Method::PATCH,
                &existing_uri,
                Some(json!({"note": "changed", "photo": photo})),
            )
            .await;
        assert_eq!(status, expected, "update with {case}: {response}");
    }
    let (status, response) = api
        .upload(
            existing["id"].as_str().unwrap(),
            "photo",
            &jpeg_start(2_000),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{response}");
    let (status, response) = api
        .upload(
            existing["id"].as_str().unwrap(),
            "picture",
            &synthetic::png(10, 10),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "no photo field: {response}"
    );

    // Nothing was written: no Meal Log, no change, no file, and no request_id consumed.
    assert_eq!(api.day("2026-10-29").await, Vec::<Value>::new());
    assert_eq!(api.day("2026-10-28").await, [existing]);
    assert_eq!(api.photo_files(), Vec::<String>::new());
    let logged = api
        .logged(with_photo(
            at("2026-10-29T12:00:00+01:00", "rejected"),
            &synthetic::png(20, 10),
        ))
        .await;
    assert_eq!(logged["photo"]["width"], 20);
}

#[tokio::test]
async fn a_retried_log_with_a_photo_replays_and_another_photo_conflicts() {
    let api = Nutrition::start().await;
    let body = with_photo(breakfast("photo-retry"), &synthetic::png(640, 480));
    let meal_log = api.logged(body.clone()).await;

    let (status, replayed) = api.log(body.clone()).await;
    assert_eq!(status, StatusCode::OK, "{replayed}");
    assert_eq!(replayed, json!({"meal_log": meal_log, "replayed": true}));
    assert_eq!(api.photo_files().len(), 2, "the retry stored nothing");

    for changed in [
        with_photo(breakfast("photo-retry"), &synthetic::png(641, 480)),
        breakfast("photo-retry"),
    ] {
        let (status, response) = api.log(changed).await;
        assert_eq!(status, StatusCode::CONFLICT, "{response}");
    }
    assert_eq!(api.photo_files().len(), 2);
}

#[tokio::test]
async fn a_photo_is_attached_replaced_or_removed_and_leaves_with_its_meal_log() {
    let api = Nutrition::start().await;
    let meal_log = api.logged(breakfast("no-photo")).await;
    assert_eq!(meal_log["photo"], Value::Null);
    let id = meal_log["id"].as_str().unwrap().to_owned();
    let uri = format!("/api/nutrition/meal-logs/{id}");

    // The app uploads a file as multipart.
    let (status, attached) = api.upload(&id, "photo", &synthetic::png(1000, 800)).await;
    assert_eq!(status, StatusCode::OK, "{attached}");
    assert_eq!(
        (&attached["portions"], &attached["note"]),
        (&meal_log["portions"], &meal_log["note"])
    );
    let first = attached["photo"].clone();
    assert_eq!(
        (&first["width"], &first["height"]),
        (&json!(1000), &json!(800))
    );

    // The agent replaces it with base64; the replaced files are removed.
    let (status, replaced) = api
        .send(
            Method::PATCH,
            &uri,
            Some(json!({"photo": synthetic::base64(&synthetic::jpeg_with_gps(600, 600, 1))})),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{replaced}");
    let second = replaced["photo"].clone();
    assert_ne!(second["id"], first["id"]);
    let second_id = second["id"].as_str().unwrap();
    assert_eq!(
        api.photo_files(),
        [
            format!("{second_id}.avif"),
            format!("{second_id}.thumbnail.avif")
        ]
    );
    for url in [&first["url"], &first["thumbnail"]["url"]] {
        assert_eq!(
            api.media(url.as_str().unwrap()).await.0,
            StatusCode::NOT_FOUND
        );
    }

    // Other corrections keep the photo; null removes it.
    let (_, noted) = api
        .send(Method::PATCH, &uri, Some(json!({"note": "late"})))
        .await;
    assert_eq!(noted["photo"], second);
    let (status, removed) = api
        .send(Method::PATCH, &uri, Some(json!({"photo": null})))
        .await;
    assert_eq!(status, StatusCode::OK, "{removed}");
    assert_eq!(removed["photo"], Value::Null);
    assert_eq!(api.photo_files(), Vec::<String>::new());
    assert_eq!(
        api.media(second["url"].as_str().unwrap()).await.0,
        StatusCode::NOT_FOUND
    );

    // Deleting the Meal Log deletes its files.
    let (_, attached) = api.upload(&id, "photo", &synthetic::png(50, 50)).await;
    assert_eq!(api.photo_files().len(), 2);
    let (status, _) = api.send(Method::DELETE, &uri, None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(api.photo_files(), Vec::<String>::new());
    assert_eq!(
        api.media(attached["photo"]["url"].as_str().unwrap())
            .await
            .0,
        StatusCode::NOT_FOUND
    );

    let (status, missing) = api.upload(&id, "photo", &synthetic::png(50, 50)).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{missing}");
    assert_eq!(api.photo_files(), Vec::<String>::new());
    for uri in [
        "/api/nutrition/meal-photos/not-a-photo",
        "/api/nutrition/meal-photos/..%2Fapp.db",
        "/api/nutrition/meal-photos/3f2b8c1e-0000-4000-8000-000000000000/thumbnail",
    ] {
        assert_eq!(api.media(uri).await.0, StatusCode::NOT_FOUND, "{uri}");
    }
}

#[tokio::test]
async fn photo_media_requires_the_api_key() {
    let api = Nutrition::start().await;
    let meal_log = api
        .logged(with_photo(breakfast("photo-auth"), &synthetic::png(40, 30)))
        .await;
    let (status, _) = send_request_with_method(
        api.app.clone(),
        meal_log["photo"]["url"].as_str().unwrap(),
        Method::GET,
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

impl Nutrition {
    async fn classify(&self, food_id: i64, nova_class: Value) -> (StatusCode, Value) {
        self.send(
            Method::PUT,
            &format!("/api/nutrition/foods/{food_id}/nova-class"),
            Some(json!({"nova_class": nova_class})),
        )
        .await
    }

    /// The NOVA Class search and the Food read report for `food_id`, found by `query`.
    async fn read_class(&self, query: &str, food_id: i64) -> (Value, Value) {
        let (status, search) = self
            .get(&format!("/api/nutrition/foods?query={query}"))
            .await;
        assert_eq!(status, StatusCode::OK, "{search}");
        let found = search["foods"]
            .as_array()
            .unwrap()
            .iter()
            .find(|food| food["food_id"] == food_id)
            .unwrap_or_else(|| panic!("{food_id} is missing from {search}"))["nova_class"]
            .clone();
        let (status, food) = self.get(&format!("/api/nutrition/foods/{food_id}")).await;
        assert_eq!(status, StatusCode::OK, "{food}");
        (found, food["nova_class"].clone())
    }
}

#[tokio::test]
async fn a_food_nova_class_is_set_changed_read_back_and_survives_a_reimport() {
    let mut api = Nutrition::start().await;
    assert_eq!(
        api.read_class("skyr", 1693).await,
        (Value::Null, Value::Null)
    );

    let (status, classified) = api.classify(1693, json!(3)).await;
    assert_eq!(status, StatusCode::OK, "{classified}");
    assert_eq!(
        classified,
        json!({
            "food_id": 1693, "name_da": "Skyr, 0.2 % fedt", "name_en": "Skyr, 0.2 % fat",
            "nova_class": 3, "classified_portions": 0,
        })
    );
    assert_eq!(api.read_class("skyr", 1693).await, (json!(3), json!(3)));
    let (status, changed) = api.classify(1693, json!(1)).await;
    assert_eq!(status, StatusCode::OK, "{changed}");
    assert_eq!(api.read_class("skyr", 1693).await, (json!(1), json!(1)));
    assert_eq!(
        api.read_class("chicken", 907).await,
        (Value::Null, Value::Null)
    );

    // The class is the user's, kept in app.db: a rebuilt Food database keeps it.
    api.reimport("2", second_dataset()).await;
    assert_eq!(api.read_class("skyr", 1693).await, (json!(1), json!(1)));
}

#[tokio::test]
async fn invalid_classifications_are_refused_and_change_nothing() {
    let api = Nutrition::start().await;

    for (food_id, nova_class, expected) in [
        (1693, json!(0), StatusCode::BAD_REQUEST),
        (1693, json!(5), StatusCode::BAD_REQUEST),
        (1693, json!(2.5), StatusCode::BAD_REQUEST),
        (1693, json!("1"), StatusCode::BAD_REQUEST),
        (1693, Value::Null, StatusCode::BAD_REQUEST),
        (0, json!(1), StatusCode::BAD_REQUEST),
        (4242, json!(1), StatusCode::NOT_FOUND),
    ] {
        let (status, body) = api.classify(food_id, nova_class.clone()).await;
        assert_eq!(status, expected, "{food_id} {nova_class}: {body}");
    }
    assert_eq!(
        api.read_class("skyr", 1693).await,
        (Value::Null, Value::Null)
    );
}

#[tokio::test]
async fn classifying_a_food_fills_its_unclassified_past_portions_and_later_portions_keep_their_class()
 {
    let mut api = Nutrition::start().await;
    let breakfast = api.logged(breakfast("breakfast-1")).await;
    assert_eq!(
        (
            &breakfast["portions"][0]["nova_class"],
            &breakfast["portions"][1]["nova_class"]
        ),
        (&Value::Null, &Value::Null)
    );

    // First eaten, then classified: the Portion just logged takes the class.
    let (_, classified) = api.classify(1693, json!(1)).await;
    assert_eq!(classified["classified_portions"], 1);
    let filled = &api.day("2026-10-29").await[0];
    assert_eq!(
        (
            &filled["portions"][0]["nova_class"],
            &filled["portions"][1]["nova_class"]
        ),
        (&json!(1), &Value::Null)
    );

    // A changed class applies from the next Portion; classified Portions keep their snapshot.
    let (_, changed) = api.classify(1693, json!(2)).await;
    assert_eq!(changed["classified_portions"], 0);
    let later = api.logged(at("2026-10-29T20:00:00+01:00", "skyr-2")).await;
    assert_eq!(later["portions"][0]["nova_class"], 2);
    let meal_logs = api.day("2026-10-29").await;
    assert_eq!(meal_logs[0]["portions"][0]["nova_class"], 1);

    // A FoodID only names the same Food within one dataset version.
    api.reimport("2", second_dataset()).await;
    let (_, chicken) = api.classify(907, json!(1)).await;
    assert_eq!(chicken["classified_portions"], 0);
    assert_eq!(
        api.day("2026-10-29").await[0]["portions"][1]["nova_class"],
        Value::Null
    );
}
