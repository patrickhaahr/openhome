use axum::Router;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use tower::ServiceExt;

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
