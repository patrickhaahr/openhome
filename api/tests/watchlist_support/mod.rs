#![allow(dead_code)]

use crate::common::send_request_with_method;
use axum::Router;
use axum::http::Method;
use axum::http::StatusCode;
use serde_json::Value;
use serde_json::json;

pub(super) async fn request(
    app: &Router,
    method: Method,
    path: &str,
    body: Value,
) -> (StatusCode, Value) {
    send_request_with_method(app.clone(), path, method, Some(body), Some("test-api-key")).await
}

pub(super) async fn owner(app: &Router, name: &str) -> Value {
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

pub(super) fn entry_path(owner: &Value, entry: &Value) -> String {
    format!(
        "/api/household-users/{}/watchlist/entries/{}",
        owner["id"].as_str().unwrap(),
        entry["id"].as_str().unwrap()
    )
}
