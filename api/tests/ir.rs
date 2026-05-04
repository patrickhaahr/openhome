mod common;

use axum::Router;
use http::StatusCode;
use serde_json::json;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn ir_test_app(service: openhome_api::services::ir::IrService) -> Router {
    let state = common::create_mock_state_with_ir(service);

    Router::new()
        .merge(openhome_api::routes::ir::router())
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
async fn test_status_endpoint_returns_503_when_service_not_configured() {
    let app = common::test_app().await;

    let (status, body) = common::send_request(app, "/api/ir", Some("test-api-key")).await;

    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["error"], "IR service is not configured");
}

#[tokio::test]
async fn test_send_endpoint_returns_400_when_command_missing() {
    let app = common::test_app().await;

    let (status, body) = common::send_request_with_method(
        app,
        "/api/ir/send",
        http::Method::POST,
        Some(json!({})),
        Some("test-api-key"),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], "command is required");
}

#[tokio::test]
async fn test_status_endpoint_returns_device_status() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            "IR remote ready\n\nAvailable commands: bluetooth, optical, mute, volume-up, volume-down, power, test\n",
        ))
        .mount(&mock_server)
        .await;

    let service = openhome_api::services::ir::IrService::new(&mock_server.uri()).unwrap();
    let app = ir_test_app(service);

    let (status, body) = common::send_request(app, "/api/ir", Some("test-api-key")).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["message"], "IR remote ready");
    assert_eq!(
        body["available_commands"],
        json!([
            "bluetooth",
            "optical",
            "mute",
            "volume-up",
            "volume-down",
            "power",
            "test"
        ])
    );
}

#[tokio::test]
async fn test_status_endpoint_returns_503_when_device_is_unavailable() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(503).set_body_string("IR bridge offline"))
        .mount(&mock_server)
        .await;

    let service = openhome_api::services::ir::IrService::new(&mock_server.uri()).unwrap();
    let app = ir_test_app(service);

    let (status, body) = common::send_request(app, "/api/ir", Some("test-api-key")).await;

    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["error"], "IR bridge offline");
}

#[tokio::test]
async fn test_send_endpoint_proxies_command_to_device() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/send"))
        .and(query_param("command", "mute"))
        .respond_with(ResponseTemplate::new(200).set_body_string("Sent command: mute"))
        .mount(&mock_server)
        .await;

    let service = openhome_api::services::ir::IrService::new(&mock_server.uri()).unwrap();
    let app = ir_test_app(service);

    let (status, body) = common::send_request_with_method(
        app,
        "/api/ir/send",
        http::Method::POST,
        Some(json!({ "command": "mute" })),
        Some("test-api-key"),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["message"], "Sent command: mute");
}

#[tokio::test]
async fn test_send_endpoint_returns_404_for_unknown_command() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/send"))
        .and(query_param("command", "party"))
        .respond_with(ResponseTemplate::new(404).set_body_string("Unknown command 'party'"))
        .mount(&mock_server)
        .await;

    let service = openhome_api::services::ir::IrService::new(&mock_server.uri()).unwrap();
    let app = ir_test_app(service);

    let (status, body) =
        common::send_request(app, "/api/ir/send?command=party", Some("test-api-key")).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"], "Unknown command 'party'");
}
