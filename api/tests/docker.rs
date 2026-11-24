mod common;

use axum::Router;
use common::{memory_noop_db, send_request, send_request_with_method};
use http::StatusCode;
use openhome_api::AppState;
use openhome_api::auth::{ApiKey, auth_middleware};
use openhome_api::routes::{
    adguard::router as adguard_router, docker::router as docker_router,
    feeds::router as feeds_router, fitness::router as fitness_router,
    health::router as health_router, ir::router as ir_router,
    switchbot::router as switchbot_router, timeline::router as timeline_router,
    training::router as training_router,
};
use openhome_api::services::docker::DockerService;
use serde_json::json;
use sqlx::SqlitePool;

async fn test_app_with_docker() -> Router {
    let api_key = ApiKey::new("test-api-key".to_string());

    let db = SqlitePool::connect(":memory:").await.unwrap();
    sqlx::migrate!("./migrations").run(&db).await.unwrap();

    let state = AppState {
        db,
        noop_db: memory_noop_db().await,
        adguard_service: None,
        docker_service: DockerService::new().ok(),
        ir_service: None,
        switchbot_service: None,
        docker_cache: std::sync::Arc::new(tokio::sync::Mutex::new(
            openhome_api::DockerCache::default(),
        )),
    };

    health_router()
        .merge(adguard_router())
        .merge(docker_router())
        .merge(feeds_router())
        .merge(fitness_router())
        .merge(ir_router())
        .merge(switchbot_router())
        .merge(timeline_router())
        .merge(training_router())
        .with_state(state)
        .layer(axum::middleware::from_fn(move |req, next| {
            auth_middleware(req, next, api_key.clone())
        }))
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
