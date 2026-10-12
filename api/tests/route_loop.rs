mod common;

use axum::{
    Router,
    http::{Method, StatusCode},
};
use common::send_request_with_method;
use openhome_api::{
    auth::{ApiKey, auth_middleware},
    routes,
    services::{
        map::MapOrigin,
        route_loop::{LoopPlanner, ProviderEndpoint},
    },
};
use serde_json::{Value, json};
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{body_partial_json, method, path},
};

/// The unrelated central Copenhagen demo origin.
const ORIGIN: [f64; 2] = [12.5692, 55.6869];

async fn app(origin: Option<MapOrigin>, provider: Option<&MockServer>) -> Router {
    let (_, state) = common::test_app_with_db().await;
    let endpoint = provider.map(|server| {
        ProviderEndpoint::parse(Some(&format!("{}/ors", server.uri())))
            .unwrap()
            .unwrap()
    });
    let planner = LoopPlanner::new(origin, endpoint).unwrap();
    routes::route_loop::router(planner)
        .with_state(state)
        .layer(axum::middleware::from_fn(|req, next| {
            auth_middleware(req, next, ApiKey::new("test-api-key".to_string()))
        }))
}

fn demo() -> Option<MapOrigin> {
    MapOrigin::parse(None, None, Some("true")).unwrap()
}

async fn generate(app: Router, body: Value) -> (StatusCode, Value) {
    send_request_with_method(
        app,
        "/api/routes/loops",
        Method::POST,
        Some(body),
        Some("test-api-key"),
    )
    .await
}

fn walk(target_meters: u32) -> Value {
    json!({ "activity": "walking", "target_meters": target_meters })
}

/// A closed rectangle starting at `start`, `side` metres north and east, sampled every ~50 m.
fn rectangle(start: [f64; 2], side_meters: u32) -> Vec<[f64; 2]> {
    let steps = side_meters.div_ceil(50) as usize;
    let side = f64::from(side_meters);
    let north = side / 111_195.0;
    let east = side / (111_195.0 * start[1].to_radians().cos());
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

fn trip(coordinates: &[[f64; 2]], summary_meters: f64) -> Value {
    json!({
        "type": "FeatureCollection",
        "features": [{
            "type": "Feature",
            "geometry": {
                "type": "LineString",
                // The provider may append elevation; only longitude and latitude are kept.
                "coordinates": coordinates.iter().map(|[lon, lat]| json!([lon, lat, 12.0])).collect::<Vec<_>>()
            },
            "properties": { "summary": { "distance": summary_meters, "duration": 3600.0 } }
        }]
    })
}

async fn provider(response: ResponseTemplate) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(response)
        .expect(1)
        .mount(&server)
        .await;
    server
}

#[tokio::test]
async fn authenticates_and_reports_missing_origin_or_provider_without_contacting_it() {
    let app_without_origin = app(None, None).await;
    let (status, _) = send_request_with_method(
        app_without_origin.clone(),
        "/api/routes/loops",
        Method::POST,
        Some(walk(5000)),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, body) = generate(app_without_origin, walk(5000)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        json!({ "state": "unavailable", "reason": "missing-origin" })
    );

    let (_, body) = generate(app(demo(), None).await, walk(5000)).await;
    assert_eq!(
        body,
        json!({ "state": "unavailable", "reason": "missing-provider" })
    );
}

#[tokio::test]
async fn generates_a_measured_walking_loop_through_the_pedestrian_provider() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/ors/v2/directions/foot-walking/geojson"))
        .and(body_partial_json(json!({
            "coordinates": [ORIGIN],
            "options": { "round_trip": { "length": 5000 }, "avoid_features": ["ferries"] }
        })))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(trip(&rectangle(ORIGIN, 1250), 4990.0)),
        )
        .expect(1)
        .mount(&server)
        .await;

    let (status, body) = generate(app(demo(), Some(&server)).await, walk(5000)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["state"], "generated");
    let route = &body["route"];
    assert_eq!(route["activity"], "walking");
    assert_eq!(route["target_meters"], 5000);
    let distance = route["distance_meters"].as_f64().unwrap();
    assert!((distance - 5000.0).abs() < 10.0, "{distance}");
    assert_eq!(route["deviation_meters"], distance - 5000.0);
    assert_eq!(route["tolerance_percent"], 10.0);
    assert_eq!(route["within_tolerance"], true);
    assert_eq!(route["geometry"]["type"], "LineString");
    let coordinates = route["geometry"]["coordinates"].as_array().unwrap();
    assert_eq!(coordinates[0], json!(ORIGIN));
    assert_eq!(coordinates.first(), coordinates.last());
}

#[tokio::test]
async fn labels_a_loop_outside_the_target_tolerance_as_a_deviation() {
    // A 5.8 km loop for a 5 km target is 16% long: shown, but never as matching.
    let server =
        provider(ResponseTemplate::new(200).set_body_json(trip(&rectangle(ORIGIN, 1450), 5800.0)))
            .await;
    let (_, body) = generate(app(demo(), Some(&server)).await, walk(5000)).await;
    assert_eq!(body["state"], "generated");
    assert_eq!(body["route"]["within_tolerance"], false);
    let percent = body["route"]["deviation_percent"].as_f64().unwrap();
    assert!((percent - 16.0).abs() < 0.3, "{percent}");
}

#[tokio::test]
async fn rejects_loops_that_do_not_start_at_the_origin_close_or_fit_the_target() {
    let far_start = [ORIGIN[0] + 0.01, ORIGIN[1]];
    let mut open = rectangle(ORIGIN, 1250);
    open.truncate(open.len() - 5);
    for (coordinates, summary) in [
        (rectangle(far_start, 1250), 5000.0),
        (open.clone(), 4750.0),
        (rectangle(ORIGIN, 500), 2000.0),
    ] {
        let server =
            provider(ResponseTemplate::new(200).set_body_json(trip(&coordinates, summary))).await;
        let (_, body) = generate(app(demo(), Some(&server)).await, walk(5000)).await;
        assert_eq!(
            body,
            json!({ "state": "unavailable", "reason": "unsuitable-route" })
        );
    }
}

#[tokio::test]
async fn rejects_invalid_provider_responses_without_inventing_a_route() {
    let valid = trip(&rectangle(ORIGIN, 1250), 5000.0);
    let mut point = valid.clone();
    point["features"][0]["geometry"]["type"] = json!("Point");
    let mut out_of_range = valid.clone();
    out_of_range["features"][0]["geometry"]["coordinates"][3] = json!([181.0, 55.0]);
    let mut disagreeing = valid.clone();
    disagreeing["features"][0]["properties"]["summary"]["distance"] = json!(9000.0);
    let mut two = valid.clone();
    two["features"] = json!([valid["features"][0], valid["features"][0]]);
    for response in [
        point,
        out_of_range,
        disagreeing,
        two,
        json!({ "features": [] }),
        json!({ "type": "FeatureCollection" }),
    ] {
        let server = provider(ResponseTemplate::new(200).set_body_json(response)).await;
        let (_, body) = generate(app(demo(), Some(&server)).await, walk(5000)).await;
        assert_eq!(
            body,
            json!({ "state": "unavailable", "reason": "provider-unavailable" })
        );
    }
}

#[tokio::test]
async fn reports_no_route_refusal_and_outage_outcomes_without_upstream_text() {
    for (response, reason) in [
        (
            ResponseTemplate::new(404).set_body_json(
                json!({ "error": { "code": 2009, "message": "synthetic-upstream-detail" } }),
            ),
            "no-route",
        ),
        (
            ResponseTemplate::new(404).set_body_json(json!({ "error": { "code": 2010 } })),
            "no-route",
        ),
        (
            ResponseTemplate::new(400).set_body_json(json!({ "error": { "code": 2003 } })),
            "provider-unavailable",
        ),
        (
            ResponseTemplate::new(502).set_body_string("synthetic-upstream-detail"),
            "provider-unavailable",
        ),
    ] {
        let server = provider(response).await;
        let (status, body) = generate(app(demo(), Some(&server)).await, walk(5000)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, json!({ "state": "unavailable", "reason": reason }));
    }
}

#[tokio::test]
async fn rejects_invalid_requests_before_contacting_the_provider() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&server)
        .await;
    let app = app(demo(), Some(&server)).await;
    for (body, message) in [
        (
            json!({ "activity": "running", "target_meters": 5000 }),
            "Unsupported activity; only walking loops are available",
        ),
        (
            walk(999),
            "Target distance must be a whole number of metres between 1000 and 25000",
        ),
        (
            json!({ "activity": "walking", "target_meters": 5000.5 }),
            "Target distance must be a whole number of metres between 1000 and 25000",
        ),
        (json!({ "activity": "walking" }), "Invalid loop request"),
    ] {
        let (status, response) = generate(app.clone(), body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(response["error"], message);
    }
}
