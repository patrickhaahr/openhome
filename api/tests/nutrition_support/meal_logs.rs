#![allow(dead_code)]

use crate::{
    common::food_workbook::first_dataset, common::food_workbook::sha256,
    common::send_request_with_method, common::test_app_with_db,
};
use axum::{Router, body::Body, http::Method, http::Request, http::StatusCode, http::header};
use openhome_api::{
    AppState, auth::ApiKey, auth::auth_middleware, db, routes,
    services::nutrition::food_database::DatasetPin,
    services::nutrition::food_database::FoodDatabase, services::nutrition::meal_photos::MealPhotos,
    services::nutrition::open_food_facts::OpenFoodFacts,
};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use sqlx::{Sqlite, pool::PoolConnection};
use tempfile::TempDir;
use tower::ServiceExt;
use wiremock::{Mock, MockServer, ResponseTemplate, matchers::method, matchers::path};

pub(super) const API_KEY: &str = "test-api-key";

pub(super) struct Nutrition {
    pub(super) server: MockServer,
    pub(super) dir: TempDir,
    pub(super) state: AppState,
    pub(super) app: Router,
    pub(super) first: Vec<u8>,
}

impl Nutrition {
    pub(super) async fn start() -> Self {
        let server = MockServer::start().await;
        let dir = TempDir::new().unwrap();
        let (_, state) = test_app_with_db().await;
        let first = first_dataset();
        let app = Self::import(&server, &dir, &state, "1", first.clone()).await;
        Self {
            server,
            dir,
            state,
            app,
            first,
        }
    }

    pub(super) async fn start_file() -> Self {
        let server = MockServer::start().await;
        let dir = TempDir::new().unwrap();
        let pool = db::connect_sqlite(&app_db_url(&dir)).await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let state = AppState {
            db: pool,
            noop_db: sqlx::SqlitePool::connect_lazy("sqlite::memory:").unwrap(),
            adguard_service: None,
            docker_service: None,
            ir_service: None,
            switchbot_service: None,
            docker_cache: std::sync::Arc::new(tokio::sync::Mutex::new(
                openhome_api::DockerCache::default(),
            )),
        };
        let first = first_dataset();
        let app = Self::import(&server, &dir, &state, "1", first.clone()).await;
        Self {
            server,
            dir,
            state,
            app,
            first,
        }
    }

    pub(super) async fn import(
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

    pub(super) async fn reimport(&mut self, version: &str, bytes: Vec<u8>) {
        self.app = Self::import(&self.server, &self.dir, &self.state, version, bytes).await;
    }

    pub(super) async fn rebuild(&mut self) {
        let url = format!("sqlite:{}", self.dir.path().join("food.db").display());
        let pool = db::connect_sqlite(&url).await.unwrap();
        sqlx::query("DELETE FROM dataset")
            .execute(&pool)
            .await
            .unwrap();
        pool.close().await;
        self.reimport("1", self.first.clone()).await;
    }

    pub(super) async fn send(
        &self,
        method: Method,
        uri: &str,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        send_request_with_method(self.app.clone(), uri, method, body, Some(API_KEY)).await
    }

    pub(super) async fn get(&self, uri: &str) -> (StatusCode, Value) {
        self.send(Method::GET, uri, None).await
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

    pub(super) async fn day(&self, day: &str) -> Vec<Value> {
        let (status, body) = self
            .get(&format!("/api/nutrition/meal-logs?day={day}"))
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        body["meal_logs"].as_array().unwrap().clone()
    }

    pub(super) async fn media(&self, uri: &str) -> (StatusCode, Option<String>, Vec<u8>) {
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

    pub(super) async fn upload(&self, id: &str, field: &str, bytes: &[u8]) -> (StatusCode, Value) {
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

    pub(super) fn photo_files(&self) -> Vec<String> {
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

pub(super) const BOUNDARY: &str = "openhome-test-boundary";

pub(super) mod synthetic {
    use base64::Engine;
    use image::{ImageEncoder, Rgb, RgbImage, codecs::jpeg::JpegEncoder, codecs::png::PngEncoder};

    pub const CAMERA: &[u8] = b"OpenHomeTestCam";

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

    pub fn avif_pixel(avif: &[u8], x: u32, y: u32) -> ((u32, u32), (u8, u8, u8)) {
        assert!(libavif::is_avif(avif), "not an AVIF file");
        let pixels = libavif::decode_rgb(avif).unwrap();
        let (red, green, blue, _) = pixels.pixel(x, y);
        ((pixels.width(), pixels.height()), (red, green, blue))
    }
}

pub(super) async fn open(
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

pub(super) fn photos(dir: &TempDir) -> MealPhotos {
    MealPhotos::new(dir.path().join("meal-photos"))
}

pub(super) fn app(foods: FoodDatabase, photos: MealPhotos, state: AppState) -> Router {
    let labels = OpenFoodFacts::new("http://127.0.0.1:9/".parse().unwrap()).unwrap();
    routes::nutrition::router(foods, photos, labels)
        .with_state(state)
        .layer(axum::middleware::from_fn(|request, next| {
            auth_middleware(request, next, ApiKey::new(API_KEY.into()))
        }))
}

pub(super) fn app_db_url(dir: &TempDir) -> String {
    format!("sqlite:{}", dir.path().join("app.db").display())
}

pub(super) async fn lock_app_db(dir: &TempDir) -> (PoolConnection<Sqlite>, sqlx::SqlitePool) {
    let pool = db::connect_sqlite(&app_db_url(dir)).await.unwrap();
    let mut lock = pool.acquire().await.unwrap();
    sqlx::query("BEGIN IMMEDIATE")
        .execute(&mut *lock)
        .await
        .unwrap();
    (lock, pool)
}

pub(super) async fn release(lock: PoolConnection<Sqlite>, pool: &sqlx::SqlitePool) {
    let mut lock = lock;
    sqlx::raw_sql("ROLLBACK").execute(&mut *lock).await.unwrap();
    drop(lock);
    pool.close().await;
}

pub(super) fn breakfast(request_id: &str) -> Value {
    json!({
        "eaten_at": "2026-10-29T08:15:00+01:00",
        "note": "  after the run ",
        "portions": [{"food_id": 1693, "grams": 150}, {"food_id": 907, "grams": 120}],
        "request_id": request_id,
    })
}

pub(super) fn at(eaten_at: &str, request_id: &str) -> Value {
    json!({
        "eaten_at": eaten_at,
        "portions": [{"food_id": 1693, "grams": 100}],
        "request_id": request_id,
    })
}

pub(super) fn nutrient<'a>(day: &'a Value, key: &str) -> &'a Value {
    day["nutrients"]
        .as_array()
        .unwrap()
        .iter()
        .find(|nutrient| nutrient["key"] == key)
        .unwrap_or_else(|| panic!("{key} is missing from {day}"))
}

pub(super) fn with_photo(mut body: Value, photo: &[u8]) -> Value {
    body["photo"] = json!(synthetic::base64(photo));
    body
}

impl Nutrition {
    pub(super) async fn classify(&self, food_id: i64, nova_class: Value) -> (StatusCode, Value) {
        self.send(
            Method::PUT,
            &format!("/api/nutrition/foods/{food_id}/nova-class"),
            Some(json!({"nova_class": nova_class})),
        )
        .await
    }

    pub(super) async fn read_class(&self, query: &str, food_id: i64) -> (Value, Value) {
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
