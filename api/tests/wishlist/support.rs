use crate::send_request_with_method::send_request_with_method;
use axum::Router;
use axum::http::Method;
use axum::http::StatusCode;
use serde_json::Value;
use serde_json::json;

pub(crate) async fn request(
    app: &Router,
    method: Method,
    path: &str,
    body: Value,
) -> (StatusCode, Value) {
    send_request_with_method(app.clone(), path, method, Some(body), Some("test-api-key")).await
}

pub(crate) async fn owner(app: &Router, name: &str) -> Value {
    let (status, body) = request(
        app,
        Method::POST,
        "/api/household-users",
        json!({"name":name}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body
}

pub(crate) async fn list(app: &Router, owner: &Value, name: &str) -> Value {
    let (status, body) = request(
        app,
        Method::POST,
        "/api/wishlists",
        json!({"owner_id":owner["id"],"name":name}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body
}
