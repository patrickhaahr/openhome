use axum::Router;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use rpi_api::AppState;
use rpi_api::auth::{ApiKey, auth_middleware};
use rpi_api::routes::{
    facts::router as facts_router, feeds::router as feeds_router, health::router,
    timeline::router as timeline_router,
};
use sqlx::SqlitePool;
use tower::ServiceExt;

#[allow(dead_code)]
pub async fn test_app() -> Router {
    test_app_with_db().await.0
}

pub async fn test_app_with_db() -> (Router, AppState) {
    let api_key = ApiKey::new("test-api-key".to_string());
    let api_key_clone = api_key.clone();

    let db = SqlitePool::connect(":memory:").await.unwrap();
    sqlx::migrate!("./migrations").run(&db).await.unwrap();

    let state = AppState { db: db.clone() };

    let app = router()
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
    let body = axum::body::to_bytes(response.into_body(), 1024)
        .await
        .unwrap();
    let json: serde_json::Value = if !body.is_empty() {
        serde_json::from_slice(&body).unwrap_or(serde_json::Value::Null)
    } else {
        serde_json::Value::Null
    };

    (status, json)
}
