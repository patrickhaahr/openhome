use axum::{Router, body::to_bytes};
use http::{Request, StatusCode};
use rpi_api::routes::health::router;
use tower::ServiceExt;

fn create_test_app() -> Router {
    router()
}

#[tokio::test]
async fn test_health_check_returns_ok_status() {
    let app = create_test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/health")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), 1024).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["status"], "ok");
}
