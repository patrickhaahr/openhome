mod common;

use axum::{Router, http::StatusCode};
use common::send_request;
use openhome_api::{
    auth::{ApiKey, auth_middleware},
    routes,
    services::map::MapOrigin,
};

async fn app(origin: Option<MapOrigin>) -> Router {
    let (_, state) = common::test_app_with_db().await;
    routes::map::router(origin)
        .with_state(state)
        .layer(axum::middleware::from_fn(|req, next| {
            auth_middleware(req, next, ApiKey::new("test-api-key".to_string()))
        }))
}

#[tokio::test]
async fn authenticates_and_reports_missing_origin_setup() {
    let app = app(None).await;
    let (status, _) = send_request(app.clone(), "/api/map", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, body) = send_request(app, "/api/map", Some("test-api-key")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        serde_json::json!({ "state": "unavailable", "reason": "missing-configuration" })
    );
}

#[tokio::test]
async fn serves_private_and_explicit_demo_origins_to_authenticated_clients() {
    let private = MapOrigin::parse(Some("55.5"), Some("12.25"), None).unwrap();
    let (status, body) = send_request(app(private).await, "/api/map", Some("test-api-key")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        serde_json::json!({
            "state": "configured",
            "origin": { "latitude": 55.5, "longitude": 12.25, "source": "private" }
        })
    );

    let demo = MapOrigin::parse(None, None, Some("true")).unwrap();
    let (_, body) = send_request(app(demo).await, "/api/map", Some("test-api-key")).await;
    assert_eq!(
        body,
        serde_json::json!({
            "state": "configured",
            "origin": { "latitude": 55.6869, "longitude": 12.5692, "source": "demo" }
        })
    );
}
