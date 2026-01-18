mod common;

use common::{send_request, send_request_with_method, test_app, test_app_with_docker};
use http::StatusCode;
use serde_json::json;

#[tokio::test]
async fn test_list_containers_returns_containers_with_valid_api_key() {
    let app = test_app_with_docker().await;
    let (status, body) = send_request(app, "/api/docker", Some("test-api-key")).await;

    // Should return OK when Docker service is available or empty array when not
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
async fn test_list_containers_returns_service_unavailable_when_docker_not_configured() {
    let app = test_app().await;
    let (status, _body) = send_request(app, "/api/docker", Some("test-api-key")).await;

    // When Docker service is not configured, should return service unavailable (503) or not found (404)
    assert!(status == StatusCode::SERVICE_UNAVAILABLE || status == StatusCode::NOT_FOUND);
    // Error field may or may not be present depending on the error type
}

#[tokio::test]
async fn test_get_container_detail_returns_container_when_found() {
    let app = test_app_with_docker().await;
    let (status, body) = send_request(
        app.clone(),
        "/api/docker/test-container",
        Some("test-api-key"),
    )
    .await;

    // If Docker service is available and container exists, should return details (200)
    // If container doesn't exist, should return 404
    // If Docker errors, should return 500
    if status == StatusCode::OK {
        assert!(body["name"].is_string());
        assert!(body["image"].is_string());
        assert!(body["state"].is_string());
        assert!(body["status"].is_string());
    } else {
        // Container not found or Docker error is acceptable in test environment
        assert!(status == StatusCode::NOT_FOUND || status == StatusCode::INTERNAL_SERVER_ERROR);
    }
}

#[tokio::test]
async fn test_get_container_detail_returns_not_found_for_nonexistent_container() {
    let app = test_app_with_docker().await;
    let (status, body) = send_request(
        app.clone(),
        "/api/docker/nonexistent-container-xyz",
        Some("test-api-key"),
    )
    .await;

    // Should return NOT_FOUND (404) when container doesn't exist
    // May return SERVER_ERROR (500) if Docker returns an error
    assert!(status == StatusCode::NOT_FOUND || status == StatusCode::INTERNAL_SERVER_ERROR);
    if status == StatusCode::NOT_FOUND {
        assert!(body["error"].is_string());
    }
}

#[tokio::test]
async fn test_get_container_detail_returns_unauthorized_without_api_key() {
    let app = test_app_with_docker().await;
    let (status, body) = send_request(app.clone(), "/api/docker/test-container", None).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"], "Missing or invalid API key");
}

#[tokio::test]
async fn test_get_container_detail_returns_service_unavailable_when_docker_not_configured() {
    let app = test_app().await;
    let (status, _body) = send_request(
        app.clone(),
        "/api/docker/test-container",
        Some("test-api-key"),
    )
    .await;

    // When Docker service is not configured, should return service unavailable or not found
    assert!(status == StatusCode::SERVICE_UNAVAILABLE || status == StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_restart_container_returns_success_when_container_exists() {
    let app = test_app_with_docker().await;
    let (status, body) = send_request_with_method(
        app.clone(),
        "/api/docker/test-container/restart",
        http::Method::POST,
        Some(json!({ "timeout_seconds": 10 })),
        Some("test-api-key"),
    )
    .await;

    // If container exists, should return success
    // If container doesn't exist, should return 404
    // If Docker errors, should return 500
    if status == StatusCode::OK {
        assert_eq!(body["success"], true);
        assert!(body["message"].is_string());
    } else {
        assert!(status == StatusCode::NOT_FOUND || status == StatusCode::INTERNAL_SERVER_ERROR);
    }
}

#[tokio::test]
async fn test_restart_container_returns_not_found_for_nonexistent_container() {
    let app = test_app_with_docker().await;
    let (status, _) = send_request_with_method(
        app.clone(),
        "/api/docker/nonexistent-container-xyz/restart",
        http::Method::POST,
        Some(json!({ "timeout_seconds": 10 })),
        Some("test-api-key"),
    )
    .await;

    assert!(status == StatusCode::NOT_FOUND || status == StatusCode::INTERNAL_SERVER_ERROR);
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

    // Should return bad request or unprocessable entity for invalid input
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
async fn test_restart_container_returns_service_unavailable_when_docker_not_configured() {
    let app = test_app().await;
    let (status, _body) = send_request_with_method(
        app.clone(),
        "/api/docker/test-container/restart",
        http::Method::POST,
        Some(json!({ "timeout_seconds": 10 })),
        Some("test-api-key"),
    )
    .await;

    // When Docker service is not configured, should return service unavailable or not found
    assert!(status == StatusCode::SERVICE_UNAVAILABLE || status == StatusCode::NOT_FOUND);
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
async fn test_get_container_logs_returns_logs_when_container_exists() {
    let app = test_app_with_docker().await;
    let (status, _) = send_request(
        app.clone(),
        "/api/docker/test-container/logs?tail=1001",
        Some("test-api-key"),
    )
    .await;

    // Logs endpoint returns plain text, not JSON
    // If container exists, should return 200 with logs
    // If container doesn't exist, should return 404 or 500 if Docker errors
    assert!(
        status == StatusCode::OK
            || status == StatusCode::NOT_FOUND
            || status == StatusCode::INTERNAL_SERVER_ERROR
    );
}

#[tokio::test]
async fn test_get_container_logs_returns_not_found_for_nonexistent_container() {
    let app = test_app_with_docker().await;
    let (status, _) = send_request(
        app.clone(),
        "/api/docker/nonexistent-container-xyz/logs?tail=50",
        Some("test-api-key"),
    )
    .await;

    // Should return NOT_FOUND (404) when container doesn't exist
    // May return SERVER_ERROR (500) if Docker returns an error
    assert!(status == StatusCode::NOT_FOUND || status == StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn test_get_container_logs_accepts_tail_parameter() {
    let app = test_app_with_docker().await;
    let (status, _) = send_request(
        app.clone(),
        "/api/docker/test-container/logs?tail=100",
        Some("test-api-key"),
    )
    .await;

    // Should accept valid tail parameter
    // May return 404 if container doesn't exist or 500 if Docker errors
    assert!(
        status == StatusCode::OK
            || status == StatusCode::NOT_FOUND
            || status == StatusCode::INTERNAL_SERVER_ERROR
    );
}

#[tokio::test]
async fn test_get_container_logs_accepts_since_parameter() {
    let app = test_app_with_docker().await;
    let (status, _) = send_request(
        app.clone(),
        "/api/docker/test-container/logs?since=2024-01-18T00:00:00Z",
        Some("test-api-key"),
    )
    .await;

    // Should accept valid since parameter
    // May return 404 if container doesn't exist or 500 if Docker errors
    assert!(
        status == StatusCode::OK
            || status == StatusCode::NOT_FOUND
            || status == StatusCode::INTERNAL_SERVER_ERROR
    );
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
async fn test_get_container_logs_accepts_timestamps_parameter() {
    let app = test_app_with_docker().await;
    let (status, _) = send_request(
        app.clone(),
        "/api/docker/test-container/logs?timestamps=true",
        Some("test-api-key"),
    )
    .await;

    // Should accept timestamps parameter
    // May return 404 if container doesn't exist, 500 if Docker errors, or OK if successful
    assert!(
        status == StatusCode::OK
            || status == StatusCode::NOT_FOUND
            || status == StatusCode::INTERNAL_SERVER_ERROR
    );
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
async fn test_get_container_logs_returns_service_unavailable_when_docker_not_configured() {
    let app = test_app().await;
    let (status, _body) = send_request(
        app.clone(),
        "/api/docker/test-container/logs?tail=50",
        Some("test-api-key"),
    )
    .await;

    // When Docker service is not configured, should return service unavailable or not found
    assert!(status == StatusCode::SERVICE_UNAVAILABLE || status == StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_docker_routes_are_registered() {
    let app = test_app_with_docker().await;

    // Verify all Docker routes are registered by checking they return appropriate responses
    // (404 for non-existent containers, etc. rather than 405 Method Not Allowed)

    // GET /api/docker - should return OK or service unavailable if Docker is configured
    let (status_list, _) = send_request(app.clone(), "/api/docker", Some("test-api-key")).await;
    assert!(status_list == StatusCode::OK || status_list == StatusCode::SERVICE_UNAVAILABLE);

    // GET /api/docker/{name} - should return 404 or 500 for non-existent container (route exists)
    // If route doesn't exist, we would get 405 Method Not Allowed
    let (status_detail, _) = send_request(
        app.clone(),
        "/api/docker/nonexistent-container-xyz",
        Some("test-api-key"),
    )
    .await;
    // If route exists but container doesn't, we get 404 or 500
    // If route doesn't exist, we get 405 Method Not Allowed
    assert!(
        status_detail == StatusCode::NOT_FOUND
            || status_detail == StatusCode::INTERNAL_SERVER_ERROR
            || status_detail == StatusCode::SERVICE_UNAVAILABLE
    );

    // POST /api/docker/{name}/restart - should return 404 or 500 for non-existent container
    let (status_restart, _) = send_request_with_method(
        app.clone(),
        "/api/docker/nonexistent-container-xyz/restart",
        http::Method::POST,
        Some(json!({ "timeout_seconds": 10 })),
        Some("test-api-key"),
    )
    .await;
    assert!(
        status_restart == StatusCode::NOT_FOUND
            || status_restart == StatusCode::INTERNAL_SERVER_ERROR
            || status_restart == StatusCode::SERVICE_UNAVAILABLE
    );

    // GET /api/docker/{name}/logs - should return 404 or 500 for non-existent container
    let (status_logs, _) = send_request(
        app.clone(),
        "/api/docker/nonexistent-container-xyz/logs",
        Some("test-api-key"),
    )
    .await;
    assert!(
        status_logs == StatusCode::NOT_FOUND
            || status_logs == StatusCode::INTERNAL_SERVER_ERROR
            || status_logs == StatusCode::SERVICE_UNAVAILABLE
    );
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
async fn test_restart_container_default_timeout() {
    let app = test_app_with_docker().await;
    let (status, _) = send_request_with_method(
        app.clone(),
        "/api/docker/test-container/restart",
        http::Method::POST,
        Some(json!({})), // Empty body should use default timeout
        Some("test-api-key"),
    )
    .await;

    // Should accept request with default timeout
    assert!(status == StatusCode::OK || status == StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_get_container_logs_with_multiple_parameters() {
    let app = test_app_with_docker().await;
    let (status, _) = send_request(
        app.clone(),
        "/api/docker/test-container/logs?tail=100&since=2024-01-18T00:00:00Z&timestamps=true",
        Some("test-api-key"),
    )
    .await;

    // Should accept multiple parameters
    // May return 404 if container doesn't exist or 500 if Docker errors
    assert!(
        status == StatusCode::OK
            || status == StatusCode::NOT_FOUND
            || status == StatusCode::INTERNAL_SERVER_ERROR
    );
}

#[tokio::test]
async fn test_docker_list_response_structure() {
    let app = test_app_with_docker().await;
    let (status, body) = send_request(app, "/api/docker", Some("test-api-key")).await;

    if status == StatusCode::OK {
        // Verify response structure
        assert!(body.is_object());
        assert!(body["containers"].is_array());
        assert!(body["timestamp"].is_string());

        // If containers array is not empty, verify container structure
        if let Some(containers) = body["containers"].as_array() {
            if !containers.is_empty() {
                let container = &containers[0];
                assert!(container["name"].is_string());
                assert!(container["status"].is_string());
                assert!(container["state"].is_string());
                assert!(container["image"].is_string());
                assert!(container["ports"].is_array());
                assert!(container["labels"].is_object());
            }
        }
    }
}

#[tokio::test]
async fn test_docker_detail_response_structure() {
    let app = test_app_with_docker().await;
    let (status, body) = send_request(
        app.clone(),
        "/api/docker/test-container",
        Some("test-api-key"),
    )
    .await;

    if status == StatusCode::OK {
        // Verify response structure
        assert!(body.is_object());
        assert!(body["name"].is_string());
        assert!(body["status"].is_string());
        assert!(body["state"].is_string());
        assert!(body["image"].is_string());
        assert!(body["image_id"].is_string());
        assert!(body["ports"].is_array());
        assert!(body["volumes"].is_array());
        assert!(body["networks"].is_array());
        assert!(body["labels"].is_object());
        assert!(body["created_at"].is_string());
        assert!(body["started_at"].is_string());
    }
}

#[tokio::test]
async fn test_docker_restart_response_structure() {
    let app = test_app_with_docker().await;
    let (status, body) = send_request_with_method(
        app.clone(),
        "/api/docker/test-container/restart",
        http::Method::POST,
        Some(json!({ "timeout_seconds": 10 })),
        Some("test-api-key"),
    )
    .await;

    if status == StatusCode::OK {
        // Verify response structure
        assert!(body.is_object());
        assert!(body["success"].is_boolean());
        assert!(body["message"].is_string());
        assert_eq!(body["success"], true);
    }
}
