#![allow(dead_code)]

use crate::common;
use axum::Router;
use axum::body::Body;
use axum::body::to_bytes;
use axum::http::Request;
use axum::http::StatusCode;
use openhome_api::auth::ApiKey;
use openhome_api::auth::auth_middleware;
use openhome_api::routes;
use openhome_api::services::photos::PhotoService;
use serde_json::Value;
use serde_json::json;
use tower::ServiceExt;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

pub(super) const ALBUM: &str = "00000000-0000-4000-8000-000000000001";

pub(super) const OTHER: &str = "00000000-0000-4000-8000-000000000003";

pub(super) const SECOND: &str = "00000000-0000-4000-8000-000000000004";

pub(super) async fn app(server: Option<&MockServer>) -> Router {
    app_with(server, &[ALBUM]).await
}

pub(super) async fn app_with(server: Option<&MockServer>, albums: &[&str]) -> Router {
    let (_, state) = common::test_app_with_db().await;
    let service = server.map(|server| {
        PhotoService::new(
            server.uri().parse().unwrap(),
            "private-immich-key",
            albums.iter().map(|id| id.parse().unwrap()).collect(),
        )
        .unwrap()
    });
    routes::photos::router(service)
        .with_state(state)
        .layer(axum::middleware::from_fn(|req, next| {
            auth_middleware(req, next, ApiKey::new("test-api-key".into()))
        }))
}

pub(super) async fn request(
    app: &Router,
    path: &str,
    key: bool,
) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
    let mut req = Request::builder().uri(path);
    if key {
        req = req.header("authorization", "Bearer test-api-key");
    }
    let response = app
        .clone()
        .oneshot(req.body(Body::empty()).unwrap())
        .await
        .unwrap();
    (
        response.status(),
        response.headers().clone(),
        to_bytes(response.into_body(), 20 * 1024 * 1024)
            .await
            .unwrap()
            .to_vec(),
    )
}

pub(super) fn album(assets: &[Value]) -> Value {
    json!({"id":ALBUM,"albumName":"Demo places","assets":assets,"owner":{"name":"private-person"},"hasSharedLink":true})
}

pub(super) async fn mount_album(server: &MockServer, value: Value) {
    Mock::given(method("GET"))
        .and(path(format!("/api/albums/{ALBUM}")))
        .and(header("x-api-key", "private-immich-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(value))
        .mount(server)
        .await;
}

pub(super) fn dated(id: &str, taken: &str) -> Value {
    json!({"id":id,"type":"IMAGE","localDateTime":taken,"fileCreatedAt":taken,"originalFileName":"private-name.jpg","exifInfo":{"city":"private-city"}})
}

pub(super) async fn mount(server: &MockServer, id: &str, value: Value) {
    Mock::given(method("GET"))
        .and(path(format!("/api/albums/{id}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(value))
        .mount(server)
        .await;
}
