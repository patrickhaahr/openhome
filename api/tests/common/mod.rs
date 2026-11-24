use axum::Router;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use openhome_api::AppState;
use openhome_api::auth::{ApiKey, auth_middleware};
use openhome_api::routes::{
    adguard::router as adguard_router, feeds::router as feeds_router,
    fitness::router as fitness_router, health::router as health_router, ir::router as ir_router,
    switchbot::router as switchbot_router, timeline::router as timeline_router,
    training::router as training_router,
};
use openhome_api::services::noop_push;
use sqlx::SqlitePool;
use sqlx::sqlite::SqlitePoolOptions;
use tower::ServiceExt;

#[allow(dead_code)]
pub async fn test_app() -> Router {
    test_app_with_db().await.0
}

pub async fn test_app_with_db() -> (Router, AppState) {
    let db = SqlitePool::connect(":memory:").await.unwrap();
    sqlx::migrate!("./migrations").run(&db).await.unwrap();
    test_app_with_pools(db, memory_noop_db().await)
}

pub async fn memory_noop_db() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    noop_push::initialize(&pool).await.unwrap();
    pool
}

#[allow(dead_code)]
pub fn test_app_with_pool(db: SqlitePool) -> (Router, AppState) {
    let noop_db = SqlitePool::connect_lazy("sqlite::memory:").unwrap();
    test_app_with_pools(db, noop_db)
}

pub fn test_app_with_pools(db: SqlitePool, noop_db: SqlitePool) -> (Router, AppState) {
    let api_key = ApiKey::new("test-api-key".to_string());
    let api_key_clone = api_key;

    let state = AppState {
        db,
        noop_db,
        adguard_service: None,
        docker_service: None,
        ir_service: None,
        switchbot_service: None,
        docker_cache: std::sync::Arc::new(tokio::sync::Mutex::new(
            openhome_api::DockerCache::default(),
        )),
    };

    let app = health_router()
        .merge(adguard_router())
        .merge(feeds_router())
        .merge(fitness_router())
        .merge(ir_router())
        .merge(switchbot_router())
        .merge(timeline_router())
        .merge(training_router())
        .merge(openhome_api::routes::wishlist::router(None))
        .merge(openhome_api::routes::watchlist::router(None))
        .merge(openhome_api::routes::important_dates::router())
        .with_state(state.clone())
        .layer(axum::middleware::from_fn(move |req, next| {
            auth_middleware(req, next, api_key_clone.clone())
        }));

    (app, state)
}

#[allow(dead_code)]
pub async fn send_request(
    app: Router,
    uri: &str,
    authorization: Option<&str>,
) -> (StatusCode, serde_json::Value) {
    send_request_with_method(app, uri, Method::GET, None, authorization).await
}

pub async fn send_request_with_method(
    app: Router,
    uri: &str,
    method: Method,
    body: Option<serde_json::Value>,
    authorization: Option<&str>,
) -> (StatusCode, serde_json::Value) {
    let request = if let Some(body_value) = body {
        let mut builder = Request::builder()
            .method(method)
            .uri(uri)
            .header(http::header::CONTENT_TYPE, "application/json");

        if let Some(auth) = authorization {
            builder = builder.header(http::header::AUTHORIZATION, format!("Bearer {auth}"));
        }

        builder
            .body(Body::from(serde_json::to_string(&body_value).unwrap()))
            .unwrap()
    } else {
        let mut builder = Request::builder().method(method).uri(uri);

        if let Some(auth) = authorization {
            builder = builder.header(http::header::AUTHORIZATION, format!("Bearer {auth}"));
        }

        builder.body(Body::empty()).unwrap()
    };

    let response = app.oneshot(request).await.unwrap();

    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let json: serde_json::Value = if body.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_slice(&body).unwrap_or(serde_json::Value::Null)
    };

    (status, json)
}
