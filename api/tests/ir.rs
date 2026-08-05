mod common;

use axum::Router;
use http::StatusCode;
use serde_json::json;
use wiremock::matchers::{body_string_contains, method, path};
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
        "/api/ir/edifier",
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
        .and(path("/remotes"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "remotes": {
                "edifier": ["bluetooth", "optical", "mute", "volume-up", "volume-down", "power"],
                "lgtv": ["power", "home", "hdmi-1"]
            }
        })))
        .mount(&mock_server)
        .await;

    let service = openhome_api::services::ir::IrService::new(&mock_server.uri()).unwrap();
    let app = ir_test_app(service);

    let (status, body) = common::send_request(app, "/api/ir", Some("test-api-key")).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["message"], "IR remote ready");
    assert_eq!(
        body["remotes"],
        json!({
            "edifier": ["bluetooth", "optical", "mute", "volume-up", "volume-down", "power"],
            "lgtv": ["power", "home", "hdmi-1"]
        })
    );
}

#[tokio::test]
async fn test_status_endpoint_returns_503_when_device_is_unavailable() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/remotes"))
        .respond_with(
            ResponseTemplate::new(503).set_body_json(json!({ "error": "IR bridge offline" })),
        )
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

    Mock::given(method("POST"))
        .and(path("/send"))
        .and(body_string_contains("remote=edifier"))
        .and(body_string_contains("command=mute"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({ "message": "Sent command: mute" })),
        )
        .mount(&mock_server)
        .await;

    let service = openhome_api::services::ir::IrService::new(&mock_server.uri()).unwrap();
    let app = ir_test_app(service);

    let (status, body) = common::send_request_with_method(
        app,
        "/api/ir/edifier",
        http::Method::POST,
        Some(json!({ "command": "mute" })),
        Some("test-api-key"),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["message"], "Sent command: mute");
}

#[tokio::test]
async fn test_lgtv_send_endpoint_proxies_remote_and_clean_command() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/send"))
        .and(body_string_contains("remote=lgtv"))
        .and(body_string_contains("command=hdmi-1"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({ "message": "Sent command: hdmi-1" })),
        )
        .mount(&mock_server)
        .await;

    let service = openhome_api::services::ir::IrService::new(&mock_server.uri()).unwrap();
    let app = ir_test_app(service);

    let (status, body) = common::send_request_with_method(
        app,
        "/api/ir/lgtv",
        http::Method::POST,
        Some(json!({ "command": "hdmi-1" })),
        Some("test-api-key"),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["message"], "Sent command: hdmi-1");
}

#[tokio::test]
async fn test_send_endpoint_rejects_get_requests() {
    let mock_server = MockServer::start().await;
    let service = openhome_api::services::ir::IrService::new(&mock_server.uri()).unwrap();
    let app = ir_test_app(service);

    let (status, _) =
        common::send_request(app, "/api/ir/edifier?command=power", Some("test-api-key")).await;

    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
}

#[tokio::test]
async fn test_send_endpoint_returns_404_for_unknown_command() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/send"))
        .and(body_string_contains("remote=edifier"))
        .and(body_string_contains("command=party"))
        .respond_with(
            ResponseTemplate::new(404).set_body_json(json!({ "error": "Unknown command" })),
        )
        .mount(&mock_server)
        .await;

    let service = openhome_api::services::ir::IrService::new(&mock_server.uri()).unwrap();
    let app = ir_test_app(service);

    let (status, body) = common::send_request_with_method(
        app,
        "/api/ir/edifier",
        http::Method::POST,
        Some(json!({ "command": "party" })),
        Some("test-api-key"),
    )
    .await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"], "Unknown command");
}
