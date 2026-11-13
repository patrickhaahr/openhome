#![allow(dead_code)]

pub mod binder;

use axum::Router;
use openhome_api::services::noop_push;
use openhome_api::services::nutrition::food_database::{DatasetPin, FoodDatabase};
use openhome_api::services::nutrition::meal_photos::MealPhotos;
use openhome_api::services::nutrition::open_food_facts::OpenFoodFacts;
use openhome_api::{AppState, DockerCache, auth, routes};
use openhome_mcp::{ApiKey, MCP_PATH, OpenHomeApi, parse_api_url};
use pretty_assertions::assert_eq;
use rmcp::model::{CallToolRequestParams, CallToolResult, ClientConfig};
use rmcp::service::RunningService;
use rmcp::transport::{
    StreamableHttpClientTransport, streamable_http_client::StreamableHttpClientTransportConfig,
    streamable_http_server::StreamableHttpServerConfig,
};
use rmcp::{RoleClient, ServiceExt};
use serde_json::{Value, json};
use sqlx::SqlitePool;
use sqlx::sqlite::SqlitePoolOptions;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;

pub const API_KEY: &str = "test-api-key";
pub const SOURCE: &str = "81906e30-187d-4546-8f8a-9949b82d62fa";

pub async fn serve(app: Router) -> SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    address
}

/// An in-memory database. One connection, because every in-memory connection is its own database.
pub async fn memory_db() -> SqlitePool {
    SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap()
}

/// The API's training routes behind its API-key layer, over a NOOP mirror where both strap
/// namespaces pushed 2026-09-14 (the real rows: imported strain 3.1, computed strain 0.0, and an
/// evening NOOP Workout) and a training log with one Workout that day.
pub async fn start_api() -> String {
    start_api_with_foods(food_database(true).await).await
}

/// The API routes over `foods`, for Food database reads.
pub async fn start_api_with_foods(foods: FoodDatabase) -> String {
    let labels = open_food_facts().await;
    let catalogue = binder::optcg().await;
    let db = training_log().await;
    binder::store_rates(&db).await;
    let state = AppState {
        db,
        noop_db: noop_mirror().await,
        adguard_service: None,
        docker_service: None,
        ir_service: None,
        switchbot_service: None,
        docker_cache: Arc::new(tokio::sync::Mutex::new(DockerCache::default())),
    };
    let api_key = auth::ApiKey::new(API_KEY.to_string());
    let app = routes::training::router()
        .merge(routes::wishlist::router(None))
        .merge(routes::watchlist::router(None))
        .merge(routes::cards::router(catalogue.clone()))
        .merge(routes::binder::router(catalogue))
        .merge(routes::important_dates::router())
        .merge(routes::nutrition::router(
            foods,
            MealPhotos::new(Path::new(env!("CARGO_TARGET_TMPDIR")).join("meal-photos")),
            labels,
        ))
        .with_state(state)
        .layer(axum::middleware::from_fn(move |request, next| {
            auth::auth_middleware(request, next, api_key.clone())
        }));
    format!("http://{}", serve(app).await)
}

/// The barcode of the synthetic product: a restricted-circulation GS1 number, never a package.
pub const SYNTHETIC_BARCODE: &str = "2001234567893";

/// Open Food Facts on loopback: it knows one synthetic skyr and nothing else.
pub async fn open_food_facts() -> OpenFoodFacts {
    let provider = Router::new().route(
        "/api/v2/product/{barcode}",
        axum::routing::get(
            |axum::extract::Path(barcode): axum::extract::Path<String>| async move {
                if barcode != SYNTHETIC_BARCODE {
                    return (
                        axum::http::StatusCode::NOT_FOUND,
                        axum::Json(json!({"code": barcode, "status": 0})),
                    );
                }
                let product = json!({
                    "product_name": "Vanilla Skyr",
                    "brands": "Synthetic Dairy",
                    "generic_name": "Skyr",
                    "nova_group": 3,
                    "nutriments": {
                        "energy-kcal_100g": 76, "proteins_100g": 9.5, "carbohydrates_100g": 8.6,
                        "fat_100g": 0.4, "salt_100g": 0.1, "vitamin-d_100g": 0.000_001,
                    },
                });
                (
                    axum::http::StatusCode::OK,
                    axum::Json(json!({"code": barcode, "product": product, "status": 1})),
                )
            },
        ),
    );
    let url = format!("http://{}/", serve(provider).await);
    OpenFoodFacts::new(url.parse().unwrap()).unwrap()
}

/// A Food database: when `imported`, one complete import of the pinned dataset holding a
/// synthetic Skyr; otherwise empty, so lookups report importing.
pub async fn food_database(imported: bool) -> FoodDatabase {
    let db = memory_db().await;
    let pin = DatasetPin::pinned();
    if imported {
        sqlx::migrate!("../api/food_migrations")
            .run(&db)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO dataset (id, version, sha256, imported_at) \
             VALUES (1, ?, ?, '2026-10-30T00:00:00Z')",
        )
        .bind(&pin.version)
        .bind(&pin.sha256)
        .execute(&db)
        .await
        .unwrap();
        sqlx::raw_sql(
            "INSERT INTO parameter (parameter_id, key, name_da, name_en, group_da, group_en, unit, \
                 per_100g, sort_key) VALUES \
                 (356, 'energy_kcal', 'Energi (kcal)', 'Energy (kcal)', 'Makronæringstoffer m.m.', \
                     'Macro nutrients etc.', 'kcal', 1, 1030), \
                 (218, 'protein', 'Protein', 'Protein', 'Makronæringstoffer m.m.', \
                     'Macro nutrients etc.', 'g', 1, 1110), \
                 (252, 'waste', 'Svind', 'Waste', 'Faktorer m.m.', 'Factors etc.', '%', 0, 20010), \
                 (363, 'density', 'Massefylde', 'Density', 'Faktorer m.m.', 'Factors etc.', 'g/ml', \
                     0, 20210); \
             INSERT INTO food (food_id, name_da, name_en, food_group_id, food_group_da, food_group_en) \
                 VALUES (1693, 'Skyr, 0.2 % fedt', 'Skyr, 0.2 % fat', 3, 'Syrnede mælkeprodukter', \
                     'Fermented milk products'); \
             INSERT INTO food_value (food_id, parameter_id, value) VALUES \
                 (1693, 356, 62.0), (1693, 218, 11.0), (1693, 252, 0.0), (1693, 363, 1.03);",
        )
        .execute(&db)
        .await
        .unwrap();
    }
    let unused_cache = std::env::temp_dir().join("openhome-mcp-contract-food-dataset");
    let (foods, import) = FoodDatabase::open(db, pin, unused_cache).await.unwrap();
    assert_eq!(import.is_some(), !imported);
    foods
}

/// The NOOP mirror: both strap namespaces' 2026-09-14 rows and replacement coverage.
pub async fn noop_mirror() -> SqlitePool {
    let noop_db = memory_db().await;
    noop_push::initialize(&noop_db).await.unwrap();
    for (device, strain) in [("my-whoop", 3.1), ("my-whoop-noop", 0.0)] {
        for (stream, selector, start, end) in [
            ("dailyMetric", "day", "2026-09-13", "2026-09-27"),
            ("sleepSession", "startTs", "1789250400", "1790460000"),
            ("workout", "startTs", "1789250400", "1790460000"),
        ] {
            sqlx::query(
                "INSERT INTO batch_ledger (receiver_state_id, source_id, device_id, batch_id, \
                     stream, body_sha256, ack, accepted_at) \
                 SELECT receiver_state_id, ?, ?, ?, ?, x'00', '{}', \
                     '2026-09-26T01:51:38.318Z' FROM receiver_state",
            )
            .bind(SOURCE)
            .bind(device)
            .bind(stream)
            .bind(stream)
            .execute(&noop_db)
            .await
            .unwrap();
            sqlx::query(
                "INSERT INTO replacement (receiver_state_id, source_id, device_id, replacement_id, \
                     stream, selector, start_inclusive, end_exclusive, parts, state) \
                 SELECT receiver_state_id, ?, ?, ?, ?, ?, ?, ?, 1, 'applied' FROM receiver_state",
            )
            .bind(SOURCE)
            .bind(device)
            .bind(stream)
            .bind(stream)
            .bind(selector)
            .bind(start)
            .bind(end)
            .execute(&noop_db)
            .await
            .unwrap();
            sqlx::query(
                "INSERT INTO replacement_part (receiver_state_id, source_id, device_id, \
                     replacement_id, part, batch_id) \
                 SELECT receiver_state_id, ?, ?, ?, 1, ? FROM receiver_state",
            )
            .bind(SOURCE)
            .bind(device)
            .bind(stream)
            .bind(stream)
            .execute(&noop_db)
            .await
            .unwrap();
        }
        sqlx::query(
            "INSERT INTO daily_metric (source_id, device_id, day, strain) \
             VALUES (?, ?, '2026-09-14', ?)",
        )
        .bind(SOURCE)
        .bind(device)
        .bind(strain)
        .execute(&noop_db)
        .await
        .unwrap();
    }

    // 18:00-19:15 local time on 2026-09-14.
    sqlx::query(
        "INSERT INTO workout (source_id, device_id, start_ts, sport, end_ts, source, duration_s, \
             avg_hr, max_hr) \
         VALUES (?, 'my-whoop', 1789401600, 'Calisthenics', 1789406100, 'manual', 4500.0, 88.0, \
             141.0)",
    )
    .bind(SOURCE)
    .execute(&noop_db)
    .await
    .unwrap();
    noop_db
}

/// The training log: one Pull Workout on 2026-09-14.
pub async fn training_log() -> SqlitePool {
    let db = memory_db().await;
    sqlx::migrate!("../api/migrations").run(&db).await.unwrap();
    sqlx::raw_sql(
        "INSERT INTO workouts (id, date, name) VALUES (1, '2026-09-14', 'Pull'); \
         INSERT INTO workout_exercises (id, workout_id, exercise_id, order_index) \
             VALUES (1, 1, 11, 0); \
         INSERT INTO sets (workout_exercise_id, set_number, reps, weight_kg, rpe, notes) \
             VALUES (1, 1, 5, 12.5, 10, '+2 partials');",
    )
    .execute(&db)
    .await
    .unwrap();
    db
}

/// The adapter, answering clients that present `API_KEY` and calling `api_url` with
/// `key_for_api`.
pub async fn start_mcp(api_url: &str, key_for_api: &str) -> String {
    let api = OpenHomeApi::new(
        parse_api_url(api_url).unwrap(),
        ApiKey::new(key_for_api.to_string()).unwrap(),
    )
    .unwrap();
    let app = openhome_mcp::router(
        api,
        ApiKey::new(API_KEY.to_string()).unwrap(),
        StreamableHttpServerConfig::default(),
    );
    format!("http://{}{MCP_PATH}", serve(app).await)
}

pub async fn connect(
    mcp_url: &str,
    key: &str,
) -> anyhow::Result<RunningService<RoleClient, ClientConfig>> {
    let transport = StreamableHttpClientTransport::from_config(
        StreamableHttpClientTransportConfig::with_uri(mcp_url.to_string()).auth_header(key),
    );
    ClientConfig::default()
        .serve(transport)
        .await
        .map_err(|err| anyhow::anyhow!("MCP client failed to start: {err:?}"))
}

pub async fn call_day_tool(
    client: &RunningService<RoleClient, ClientConfig>,
    tool: &'static str,
    day: &str,
) -> CallToolResult {
    let arguments = json!({"day": day}).as_object().unwrap().clone();
    client
        .call_tool(CallToolRequestParams::new(tool).with_arguments(arguments))
        .await
        .unwrap()
}

pub async fn wishlist_call(
    client: &RunningService<RoleClient, ClientConfig>,
    tool: &'static str,
    arguments: Value,
) -> CallToolResult {
    client
        .call_tool(
            CallToolRequestParams::new(tool).with_arguments(arguments.as_object().unwrap().clone()),
        )
        .await
        .unwrap()
}

pub async fn recovery_on_day(
    client: &RunningService<RoleClient, ClientConfig>,
    day: &str,
) -> CallToolResult {
    call_day_tool(client, "recovery_on_day", day).await
}

pub async fn api_get(api_url: &str, day: &str) -> Value {
    api_get_read(api_url, day, "recovery").await
}

pub async fn api_get_read(api_url: &str, day: &str, read: &str) -> Value {
    reqwest::Client::new()
        .get(format!("{api_url}/api/training/days/{day}/{read}"))
        .bearer_auth(API_KEY)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
}

pub async fn api_get_sleep_recent(api_url: &str, n: u8) -> Value {
    reqwest::Client::new()
        .get(format!("{api_url}/api/training/sleep/recent/{n}"))
        .bearer_auth(API_KEY)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
}

pub async fn call_sleep_recent(
    client: &RunningService<RoleClient, ClientConfig>,
    n: u8,
) -> CallToolResult {
    client
        .call_tool(
            CallToolRequestParams::new("sleep_recent")
                .with_arguments(json!({"n": n}).as_object().unwrap().clone()),
        )
        .await
        .unwrap()
}
