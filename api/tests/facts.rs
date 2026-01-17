mod common;

use http::StatusCode;

#[tokio::test]
async fn test_should_return_random_fact_with_valid_api_key() {
    let app = common::test_app().await;

    let (status, body) = common::send_request(app, "/api/facts/random", Some("test-api-key")).await;

    assert_eq!(status, StatusCode::OK);
    assert!(body["text"].is_string());
    assert!(!body["text"].as_str().unwrap().is_empty());
}
