use axum::Router;
use axum::http::{Method, StatusCode};

use crate::send_request_with_method::send_request_with_method;

pub async fn send_request(
    app: Router,
    uri: &str,
    authorization: Option<&str>,
) -> (StatusCode, serde_json::Value) {
    send_request_with_method(app, uri, Method::GET, None, authorization).await
}
