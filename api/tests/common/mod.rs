use axum::Router;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use openhome_api::AppState;
use openhome_api::auth::{ApiKey, auth_middleware};
use openhome_api::routes::{
    adguard::router as adguard_router, docker::router as docker_router,
    facts::router as facts_router, feeds::router as feeds_router, health::router as health_router,
    timeline::router as timeline_router,
};
use openhome_api::services::adguard::AdguardService;
use openhome_api::services::docker::DockerService;
use sqlx::SqlitePool;
use tower::ServiceExt;

#[allow(dead_code)]
pub async fn test_app() -> Router {
    test_app_with_db().await.0
}

pub async fn test_app_with_db() -> (Router, AppState) {
    test_app_with_db_and_adguard(None).await
}

#[allow(dead_code)]
pub async fn test_app_with_adguard() -> Router {
    test_app_with_db_and_adguard(Some(true)).await.0
}

#[allow(dead_code)]
pub fn create_mock_state_with_adguard(service: AdguardService) -> AppState {
    let db = SqlitePool::connect_lazy("sqlite::memory:").unwrap();
    AppState {
        db,
        adguard_service: Some(service),
        docker_service: None,
        docker_cache: std::sync::Arc::new(tokio::sync::Mutex::new(openhome_api::DockerCache::default())),
    }
}

pub async fn test_app_with_db_and_adguard(adguard_enabled: Option<bool>) -> (Router, AppState) {
    let api_key = ApiKey::new("test-api-key".to_string());
    let api_key_clone = api_key.clone();

    let db = SqlitePool::connect(":memory:").await.unwrap();
    sqlx::migrate!("./migrations").run(&db).await.unwrap();

    let adguard_service = if adguard_enabled.unwrap_or(false) {
        Some(AdguardService::new("http://localhost:9999", "test", "test", false).unwrap())
    } else {
        None
    };

    let state = AppState {
        db: db.clone(),
        adguard_service,
        docker_service: None,
        docker_cache: std::sync::Arc::new(tokio::sync::Mutex::new(openhome_api::DockerCache::default())),
    };

    let app = health_router()
        .merge(adguard_router())
        .merge(facts_router())
        .merge(feeds_router())
        .merge(timeline_router())
        .with_state(state.clone())
        .layer(axum::middleware::from_fn(move |req, next| {
            auth_middleware(req, next, api_key_clone.clone())
        }));

    (app, state)
}

#[allow(dead_code)]
pub async fn test_app_with_docker() -> Router {
    test_app_with_docker_and_adguard(None).await.0
}

#[allow(dead_code)]
pub async fn test_app_with_docker_and_adguard(adguard_enabled: Option<bool>) -> (Router, AppState) {
    let api_key = ApiKey::new("test-api-key".to_string());
    let api_key_clone = api_key.clone();

    let db = SqlitePool::connect(":memory:").await.unwrap();
    sqlx::migrate!("./migrations").run(&db).await.unwrap();

    let adguard_service = if adguard_enabled.unwrap_or(false) {
        Some(AdguardService::new("http://localhost:9999", "test", "test", false).unwrap())
    } else {
        None
    };

    // Try to create Docker service, but allow it to be None if Docker socket is not available
    let docker_service = DockerService::new().ok();

    let state = AppState {
        db: db.clone(),
        adguard_service,
        docker_service,
        docker_cache: std::sync::Arc::new(tokio::sync::Mutex::new(openhome_api::DockerCache::default())),
    };

    let app = health_router()
        .merge(adguard_router())
        .merge(docker_router())
        .merge(facts_router())
        .merge(feeds_router())
        .merge(timeline_router())
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
    let body = axum::body::to_bytes(response.into_body(), 1024 * 100)
        .await
        .unwrap();
    let json: serde_json::Value = if !body.is_empty() {
        serde_json::from_slice(&body).unwrap_or(serde_json::Value::Null)
    } else {
        serde_json::Value::Null
    };

    (status, json)
}
