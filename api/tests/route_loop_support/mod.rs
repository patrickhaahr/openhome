#![allow(dead_code)]

use crate::common;
use crate::common::send_request_with_method;
use axum::Router;
use axum::http::Method;
use axum::http::StatusCode;
use openhome_api::AppState;
use openhome_api::auth::ApiKey;
use openhome_api::auth::auth_middleware;
use openhome_api::routes;
use openhome_api::services::map::MapOrigin;
use openhome_api::services::route_loop::LoopPlanner;
use openhome_api::services::route_loop::ProviderEndpoint;
use serde_json::Value;
use serde_json::json;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::body_partial_json;
use wiremock::matchers::method;

pub(super) const ORIGIN: [f64; 2] = [12.5692, 55.6869];

pub(super) const MAX_ATTEMPTS: u64 = 6;

pub(super) async fn app(origin: Option<MapOrigin>, provider: Option<&MockServer>) -> Router {
    let (_, state) = common::test_app_with_db().await;
    let endpoint = provider.map(|server| {
        ProviderEndpoint::parse(Some(&format!("{}/ors", server.uri())))
            .unwrap()
            .unwrap()
    });
    let planner = LoopPlanner::new(origin, endpoint).unwrap();
    app_with_state(planner, state)
}

pub(super) fn app_with_state(planner: LoopPlanner, state: AppState) -> Router {
    routes::route_loop::router(planner)
        .with_state(state)
        .layer(axum::middleware::from_fn(|req, next| {
            auth_middleware(req, next, ApiKey::new("test-api-key".to_string()))
        }))
}

pub(super) fn demo() -> Option<MapOrigin> {
    MapOrigin::parse(None, None, Some("true")).unwrap()
}

pub(super) async fn generate(app: Router, body: Value) -> (StatusCode, Value) {
    send_request_with_method(
        app,
        "/api/routes/loops",
        Method::POST,
        Some(body),
        Some("test-api-key"),
    )
    .await
}

pub(super) fn walk(meters: u32) -> Value {
    json!({
        "activity": "walking",
        "target": { "kind": "distance", "meters": meters },
        "step_length_cm": 75
    })
}

pub(super) fn rectangle(
    start: [f64; 2],
    side_meters: u32,
    [east, north]: [f64; 2],
) -> Vec<[f64; 2]> {
    let steps = side_meters.div_ceil(50) as usize;
    let side = f64::from(side_meters);
    let north = north * side / 111_195.0;
    let east = east * side / (111_195.0 * start[1].to_radians().cos());
    let corners = [
        start,
        [start[0], start[1] + north],
        [start[0] + east, start[1] + north],
        [start[0] + east, start[1]],
        start,
    ];
    let mut points = vec![start];
    for pair in corners.windows(2) {
        for step in 1..=steps {
            let t = step as f64 / steps as f64;
            points.push([
                pair[0][0] + (pair[1][0] - pair[0][0]) * t,
                pair[0][1] + (pair[1][1] - pair[0][1]) * t,
            ]);
        }
    }
    points
}

pub(super) const NORTH_EAST: [f64; 2] = [1.0, 1.0];

pub(super) const NORTH_WEST: [f64; 2] = [-1.0, 1.0];

pub(super) const SOUTH_WEST: [f64; 2] = [-1.0, -1.0];

pub(super) const SOUTH_EAST: [f64; 2] = [1.0, -1.0];

pub(super) fn square(perimeter: u32, direction: [f64; 2]) -> Value {
    trip(
        &rectangle(ORIGIN, perimeter / 4, direction),
        f64::from(perimeter),
    )
}

pub(super) fn trip(coordinates: &[[f64; 2]], summary_meters: f64) -> Value {
    json!({
        "type": "FeatureCollection",
        "features": [{
            "type": "Feature",
            "geometry": {
                "type": "LineString",
                "coordinates": coordinates.iter().map(|[lon, lat]| json!([lon, lat, 12.0])).collect::<Vec<_>>()
            },
            "properties": { "summary": { "distance": summary_meters, "duration": 3600.0 } }
        }]
    })
}

pub(super) async fn answer_seed(
    server: &MockServer,
    seed: u32,
    response: ResponseTemplate,
    times: u64,
) {
    Mock::given(method("POST"))
        .and(body_partial_json(
            json!({ "options": { "round_trip": { "seed": seed } } }),
        ))
        .respond_with(response)
        .expect(times)
        .mount(server)
        .await;
}

pub(super) async fn provider(response: ResponseTemplate, times: u64) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(response)
        .expect(times)
        .mount(&server)
        .await;
    server
}

pub(super) fn ok(body: Value) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(body)
}

pub(super) fn distances(plan: &Value) -> Vec<f64> {
    plan["loops"]
        .as_array()
        .unwrap()
        .iter()
        .map(|route| route["distance_meters"].as_f64().unwrap())
        .collect()
}
