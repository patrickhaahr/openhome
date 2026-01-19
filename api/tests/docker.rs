mod common;

use common::{send_request, send_request_with_method, test_app_with_docker};
use http::StatusCode;
use serde_json::json;

#[tokio::test]
async fn test_list_containers_returns_containers_with_valid_api_key() {
    let app = test_app_with_docker().await;
    let (status, body) = send_request(app, "/api/docker", Some("test-api-key")).await;

    assert_eq!(status, StatusCode::OK);
    assert!(body["containers"].is_array());
    assert!(body["timestamp"].is_string());
}

#[tokio::test]
async fn test_list_containers_returns_unauthorized_without_api_key() {
    let app = test_app_with_docker().await;
    let (status, body) = send_request(app, "/api/docker", None).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"], "Missing or invalid API key");
    assert_eq!(body["status"], 401);
}

#[tokio::test]
async fn test_list_containers_returns_unauthorized_with_invalid_api_key() {
    let app = test_app_with_docker().await;
    let (status, body) = send_request(app, "/api/docker", Some("wrong-api-key")).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"], "Missing or invalid API key");
    assert_eq!(body["status"], 401);
}

#[tokio::test]
async fn test_get_container_detail_returns_unauthorized_without_api_key() {
    let app = test_app_with_docker().await;
    let (status, body) = send_request(app.clone(), "/api/docker/test-container", None).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"], "Missing or invalid API key");
}

#[tokio::test]
async fn test_restart_container_returns_bad_request_for_invalid_timeout() {
    let app = test_app_with_docker().await;
    let (status, _) = send_request_with_method(
        app.clone(),
        "/api/docker/test-container/restart",
        http::Method::POST,
        Some(json!({ "timeout_seconds": -1 })),
        Some("test-api-key"),
    )
    .await;

    assert!(status == StatusCode::BAD_REQUEST || status == StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_restart_container_returns_unauthorized_without_api_key() {
    let app = test_app_with_docker().await;
    let (status, body) = send_request_with_method(
        app.clone(),
        "/api/docker/test-container/restart",
        http::Method::POST,
        Some(json!({ "timeout_seconds": 10 })),
        None,
    )
    .await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"], "Missing or invalid API key");
}

#[tokio::test]
async fn test_restart_container_rejects_get_requests() {
    let app = test_app_with_docker().await;
    let (status, _) = send_request(
        app.clone(),
        "/api/docker/test-container/restart",
        Some("test-api-key"),
    )
    .await;

    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
}

#[tokio::test]
async fn test_get_container_logs_rejects_invalid_since_parameter() {
    let app = test_app_with_docker().await;
    let (status, body) = send_request(
        app.clone(),
        "/api/docker/test-container/logs?since=not-a-timestamp",
        Some("test-api-key"),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["error"].is_string());
}

#[tokio::test]
async fn test_get_container_logs_returns_unauthorized_without_api_key() {
    let app = test_app_with_docker().await;
    let (status, body) =
        send_request(app.clone(), "/api/docker/test-container/logs?tail=50", None).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"], "Missing or invalid API key");
}

#[tokio::test]
async fn test_docker_endpoint_rejects_post_requests() {
    let app = test_app_with_docker().await;
    let (status, _) = send_request_with_method(
        app.clone(),
        "/api/docker",
        http::Method::POST,
        None,
        Some("test-api-key"),
    )
    .await;

    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
}

#[tokio::test]
async fn test_docker_endpoint_rejects_put_requests() {
    let app = test_app_with_docker().await;
    let (status, _) = send_request_with_method(
        app.clone(),
        "/api/docker",
        http::Method::PUT,
        None,
        Some("test-api-key"),
    )
    .await;

    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
}

#[tokio::test]
async fn test_docker_endpoint_rejects_delete_requests() {
    let app = test_app_with_docker().await;
    let (status, _) = send_request_with_method(
        app.clone(),
        "/api/docker",
        http::Method::DELETE,
        None,
        Some("test-api-key"),
    )
    .await;

    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
}

#[tokio::test]
async fn test_start_container_returns_unauthorized_without_api_key() {
    let app = test_app_with_docker().await;
    let (status, body) = send_request_with_method(
        app.clone(),
        "/api/docker/test-container/start",
        http::Method::POST,
        None,
        None,
    )
    .await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"], "Missing or invalid API key");
}

#[tokio::test]
async fn test_start_container_rejects_get_requests() {
    let app = test_app_with_docker().await;
    let (status, _) = send_request(
        app.clone(),
        "/api/docker/test-container/start",
        Some("test-api-key"),
    )
    .await;

    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
}

#[tokio::test]
async fn test_start_container_response_structure() {
    let app = test_app_with_docker().await;
    let (status, body) = send_request_with_method(
        app.clone(),
        "/api/docker/test-container/start",
        http::Method::POST,
        None,
        Some("test-api-key"),
    )
    .await;

    if status == StatusCode::OK {
        assert!(body.is_object());
        assert!(body["success"].is_boolean());
        assert!(body["message"].is_string());
        assert_eq!(body["success"], true);
    }
}

#[tokio::test]
async fn test_stop_container_returns_bad_request_for_invalid_timeout() {
    let app = test_app_with_docker().await;
    let (status, _) = send_request_with_method(
        app.clone(),
        "/api/docker/test-container/stop",
        http::Method::POST,
        Some(json!({ "timeout_seconds": -1 })),
        Some("test-api-key"),
    )
    .await;

    assert!(status == StatusCode::BAD_REQUEST || status == StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn test_stop_container_returns_unauthorized_without_api_key() {
    let app = test_app_with_docker().await;
    let (status, body) = send_request_with_method(
        app.clone(),
        "/api/docker/test-container/stop",
        http::Method::POST,
        Some(json!({ "timeout_seconds": 10 })),
        None,
    )
    .await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"], "Missing or invalid API key");
}

#[tokio::test]
async fn test_stop_container_rejects_get_requests() {
    let app = test_app_with_docker().await;
    let (status, _) = send_request(
        app.clone(),
        "/api/docker/test-container/stop",
        Some("test-api-key"),
    )
    .await;

    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
}

#[tokio::test]
async fn test_stop_container_response_structure() {
    let app = test_app_with_docker().await;
    let (status, body) = send_request_with_method(
        app.clone(),
        "/api/docker/test-container/stop",
        http::Method::POST,
        Some(json!({ "timeout_seconds": 10 })),
        Some("test-api-key"),
    )
    .await;

    if status == StatusCode::OK {
        assert!(body.is_object());
        assert!(body["success"].is_boolean());
        assert!(body["message"].is_string());
        assert!(body["stopped"].is_boolean());
        assert_eq!(body["success"], true);
    }
}
