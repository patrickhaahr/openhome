use axum::Router;
use axum::body::Body;
use http::{Request, StatusCode};
use rpi_api::auth::{ApiKey, auth_middleware};
use rpi_api::routes::health::router;
use tower::ServiceExt;

pub fn test_app() -> Router {
    let api_key = ApiKey::new("test-api-key".to_string());
    let api_key_clone = api_key.clone();
    router().layer(axum::middleware::from_fn(move |req, next| {
        auth_middleware(req, next, api_key_clone.clone())
    }))
}

pub async fn send_request(
    app: &Router,
    uri: &str,
    authorization: Option<&str>,
) -> (StatusCode, serde_json::Value) {
    let mut builder = Request::builder().uri(uri);

    if let Some(auth) = authorization {
        builder = builder.header(http::header::AUTHORIZATION, format!("Bearer {auth}"));
    }

    let app = app.clone();
    let response = app
        .oneshot(builder.body(Body::empty()).unwrap())
        .await
        .unwrap();

    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 1024)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    (status, json)
}
