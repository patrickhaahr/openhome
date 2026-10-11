mod common;

use axum::{Router, http::StatusCode};
use common::send_request;
use openhome_api::{
    auth::{ApiKey, auth_middleware},
    routes,
    services::weather::WeatherService,
};

async fn app(service: Option<WeatherService>) -> Router {
    let (_, state) = common::test_app_with_db().await;
    routes::weather::router(service)
        .with_state(state)
        .layer(axum::middleware::from_fn(|req, next| {
            auth_middleware(req, next, ApiKey::new("test-api-key".to_string()))
        }))
}

#[tokio::test]
async fn authenticates_weather_and_reports_absent_location_without_conditions() {
    let app = app(None).await;
    let (status, _) = send_request(app.clone(), "/api/weather", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, body) = send_request(app, "/api/weather", Some("test-api-key")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        serde_json::json!({ "state": "unavailable", "reason": "missing-configuration" })
    );
}
