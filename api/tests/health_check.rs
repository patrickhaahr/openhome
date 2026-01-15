mod common;

use http::StatusCode;

#[tokio::test]
async fn test_should_return_ok_status_with_valid_api_key() {
    let app = common::test_app();

    let (status, body) = common::send_request(&app, "/api/health", Some("test-api-key")).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ok");
}

#[tokio::test]
async fn test_should_return_unauthorized_without_api_key() {
    let app = common::test_app();

    let (status, body) = common::send_request(&app, "/api/health", None).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"], "Missing or invalid API key");
    assert_eq!(body["status"], 401);
}

#[tokio::test]
async fn test_should_return_unauthorized_with_invalid_api_key() {
    let app = common::test_app();

    let (status, body) = common::send_request(&app, "/api/health", Some("wrong-api-key")).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"], "Missing or invalid API key");
    assert_eq!(body["status"], 401);
}
