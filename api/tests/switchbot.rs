mod common;

use axum::Router;
use http::StatusCode;
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn switchbot_test_app(service: openhome_api::services::switchbot::SwitchbotService) -> Router {
    let state = common::create_mock_state_with_switchbot(service);

    Router::new()
        .merge(openhome_api::routes::switchbot::router())
        .with_state(state)
        .layer(axum::middleware::from_fn(|req, next| {
            openhome_api::auth::auth_middleware(
                req,
                next,
                openhome_api::auth::ApiKey::new("test-api-key".to_string()),
            )
        }))
}

#[tokio::test]
async fn test_on_endpoint_returns_503_when_service_not_configured() {
    let app = common::test_app().await;

    let (status, body) = common::send_request_with_method(
        app,
        "/api/lights/on",
        http::Method::POST,
        None,
        Some("test-api-key"),
    )
    .await;

    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["error"], "SwitchBot service is not configured");
}

#[tokio::test]
async fn test_on_endpoint_proxies_command_to_switchbot() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/lights/on"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({ "message": "Light switched on" })),
        )
        .mount(&mock_server)
        .await;

    let service =
        openhome_api::services::switchbot::SwitchbotService::new(&mock_server.uri()).unwrap();
    let app = switchbot_test_app(service);

    let (status, body) = common::send_request_with_method(
        app,
        "/api/lights/on",
        http::Method::POST,
        None,
        Some("test-api-key"),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["message"], "Light switched on");
}

#[tokio::test]
async fn test_off_endpoint_proxies_command_to_switchbot() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/lights/off"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({ "message": "Light switched off" })),
        )
        .mount(&mock_server)
        .await;

    let service =
        openhome_api::services::switchbot::SwitchbotService::new(&mock_server.uri()).unwrap();
    let app = switchbot_test_app(service);

    let (status, body) = common::send_request_with_method(
        app,
        "/api/lights/off",
        http::Method::POST,
        None,
        Some("test-api-key"),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["message"], "Light switched off");
}

#[tokio::test]
async fn test_endpoint_returns_503_when_switchbot_is_unavailable() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/lights/on"))
        .respond_with(
            ResponseTemplate::new(503).set_body_json(json!({ "error": "SwitchBot unavailable" })),
        )
        .mount(&mock_server)
        .await;

    let service =
        openhome_api::services::switchbot::SwitchbotService::new(&mock_server.uri()).unwrap();
    let app = switchbot_test_app(service);

    let (status, body) = common::send_request_with_method(
        app,
        "/api/lights/on",
        http::Method::POST,
        None,
        Some("test-api-key"),
    )
    .await;

    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["error"], "SwitchBot unavailable");
}

#[tokio::test]
async fn test_endpoint_rejects_get_requests() {
    let app = common::test_app().await;

    let (status, _) = common::send_request(app, "/api/lights/on", Some("test-api-key")).await;

    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
}
