#[path = "common/memory_noop_db.rs"]
mod memory_noop_db;
#[path = "common/send_request.rs"]
mod send_request;
#[path = "common/send_request_with_method.rs"]
mod send_request_with_method;
#[path = "common/test_app.rs"]
mod test_app;
#[path = "common/test_app_with_db.rs"]
mod test_app_with_db;
#[path = "common/test_app_with_pools.rs"]
mod test_app_with_pools;

use crate::send_request::send_request;
use crate::test_app::test_app;
use http::StatusCode;

#[tokio::test]
async fn test_should_return_ok_status_with_valid_api_key() {
    let app = test_app().await;

    let (status, body) = send_request(app, "/api/health", Some("test-api-key")).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ok");
}

#[tokio::test]
async fn test_should_return_unauthorized_without_api_key() {
    let app = test_app().await;

    let (status, body) = send_request(app, "/api/health", None).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"], "Missing or invalid API key");
    assert_eq!(body["status"], 401);
}

#[tokio::test]
async fn test_should_return_unauthorized_with_invalid_api_key() {
    let app = test_app().await;

    let (status, body) = send_request(app, "/api/health", Some("wrong-api-key")).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"], "Missing or invalid API key");
    assert_eq!(body["status"], 401);
}
