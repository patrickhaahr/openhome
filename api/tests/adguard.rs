mod common;

use axum::Router;
use http::StatusCode;
use serde_json::json;

#[tokio::test]
async fn test_status_endpoint_returns_503_when_service_not_configured() {
    let app = common::test_app().await;

    let (status, _body) =
        common::send_request(app, "/api/adguard/status", Some("test-api-key")).await;

    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn test_enable_endpoint_returns_503_when_service_not_configured() {
    let app = common::test_app().await;

    let (status, _body) = common::send_request_with_method(
        app,
        "/api/adguard/enable",
        http::Method::POST,
        None,
        Some("test-api-key"),
    )
    .await;

    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn test_disable_endpoint_returns_503_when_service_not_configured() {
    let app = common::test_app().await;

    let (status, _body) = common::send_request_with_method(
        app,
        "/api/adguard/disable",
        http::Method::POST,
        None,
        Some("test-api-key"),
    )
    .await;

    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn test_pause_endpoint_with_valid_body_when_service_not_configured() {
    let app = common::test_app().await;
    let body = json!({ "minutes": 5 });
    let (status, _body) = common::send_request_with_method(
        app,
        "/api/adguard/pause",
        http::Method::POST,
        Some(body),
        Some("test-api-key"),
    )
    .await;

    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn test_pause_endpoint_returns_422_for_invalid_json() {
    let app = common::test_app().await;

    let (status, _body) = common::send_request_with_method(
        app,
        "/api/adguard/pause",
        http::Method::POST,
        Some(json!("invalid json")),
        Some("test-api-key"),
    )
    .await;

    // Invalid JSON should return 422
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_undefined_endpoint_returns_404() {
    let app = common::test_app().await;

    let (status, _body) =
        common::send_request(app, "/api/adguard/undefined", Some("test-api-key")).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_status_endpoint_rejects_non_get_requests() {
    let app = common::test_app().await;

    // POST to status endpoint should return 405 Method Not Allowed
    let (status, _body) = common::send_request_with_method(
        app,
        "/api/adguard/status",
        http::Method::POST,
        None,
        Some("test-api-key"),
    )
    .await;

    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
}

#[tokio::test]
async fn test_enable_endpoint_rejects_get_requests() {
    let app = common::test_app().await;

    // GET to enable endpoint should return 405 Method Not Allowed
    let (status, _body) =
        common::send_request(app, "/api/adguard/enable", Some("test-api-key")).await;

    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
}

#[tokio::test]
async fn test_pause_endpoint_with_adguard_service() {
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let mock_server = MockServer::start().await;

    let status_response = json!({
        "version": "v1.5.0",
        "language": "en",
        "dns_addresses": [],
        "dns_port": 53,
        "http_port": 80,
        "protection_disabled_duration": 0,
        "protection_enabled": false,
        "protection_disabled_until": null,
        "dhcp_available": false,
        "running": true
    });

    Mock::given(method("POST"))
        .and(path("/control/protection"))
        .and(header("Authorization", "Basic dGVzdDp0ZXN0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(status_response.clone()))
        .mount(&mock_server)
        .await;

    Mock::given(method("GET"))
        .and(path("/control/status"))
        .respond_with(ResponseTemplate::new(200).set_body_json(status_response))
        .mount(&mock_server)
        .await;

    let service =
        rpi_api::services::adguard::AdguardService::new(&mock_server.uri(), "test", "test", false)
            .unwrap();
    let state = common::create_mock_state_with_adguard(service);

    let app = Router::new()
        .merge(rpi_api::routes::adguard::router())
        .with_state(state)
        .layer(axum::middleware::from_fn(|req, next| {
            rpi_api::auth::auth_middleware(
                req,
                next,
                rpi_api::auth::ApiKey::new("test-api-key".to_string()),
            )
        }));

    let body = json!({ "minutes": 5 });

    let (status, _body) = common::send_request_with_method(
        app,
        "/api/adguard/pause",
        http::Method::POST,
        Some(body),
        Some("test-api-key"),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn test_all_adguard_routes_are_registered() {
    let app = common::test_app().await;

    // The router should have routes defined for all adguard endpoints
    // We can verify this by checking they don't return 404
    let status_response =
        common::send_request(app.clone(), "/api/adguard/status", Some("test-api-key")).await;
    assert_ne!(status_response.0, StatusCode::NOT_FOUND);

    let enable_response = common::send_request_with_method(
        app.clone(),
        "/api/adguard/enable",
        http::Method::POST,
        None,
        Some("test-api-key"),
    )
    .await;
    assert_ne!(enable_response.0, StatusCode::NOT_FOUND);

    let disable_response = common::send_request_with_method(
        app.clone(),
        "/api/adguard/disable",
        http::Method::POST,
        None,
        Some("test-api-key"),
    )
    .await;
    assert_ne!(disable_response.0, StatusCode::NOT_FOUND);

    let pause_response = common::send_request_with_method(
        app.clone(),
        "/api/adguard/pause",
        http::Method::POST,
        Some(json!({ "minutes": 5 })),
        Some("test-api-key"),
    )
    .await;
    assert_ne!(pause_response.0, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_pause_endpoint_returns_422_when_minutes_missing() {
    let app = common::test_app().await;
    let body = json!({});

    let (status, _body) = common::send_request_with_method(
        app,
        "/api/adguard/pause",
        http::Method::POST,
        Some(body),
        Some("test-api-key"),
    )
    .await;

    // Missing required field should return 422
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_pause_endpoint_rejects_zero_minutes() {
    let app = common::test_app().await;
    let body = json!({ "minutes": 0 });

    let (status, _body) = common::send_request_with_method(
        app,
        "/api/adguard/pause",
        http::Method::POST,
        Some(body),
        Some("test-api-key"),
    )
    .await;

    // Zero minutes is not valid - should be a validation error
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
