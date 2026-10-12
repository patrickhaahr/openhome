mod common;

use std::time::Duration;

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
    Mock, MockServer, Request, ResponseTemplate,
    matchers::{body_partial_json, method, path},
};

/// The unrelated central Copenhagen demo origin.
const ORIGIN: [f64; 2] = [12.5692, 55.6869];

/// Round trips one search may request.
const MAX_ATTEMPTS: u64 = 6;

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

fn walk(meters: u32) -> Value {
    json!({
        "activity": "walking",
        "target": { "kind": "distance", "meters": meters },
        "step_length_cm": 75
    })
}

/// A closed rectangle from `start`, `side` metres per edge towards `[east, north]` signs,
/// sampled every ~50 m.
fn rectangle(start: [f64; 2], side_meters: u32, [east, north]: [f64; 2]) -> Vec<[f64; 2]> {
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

const NORTH_EAST: [f64; 2] = [1.0, 1.0];
const NORTH_WEST: [f64; 2] = [-1.0, 1.0];
const SOUTH_WEST: [f64; 2] = [-1.0, -1.0];
const SOUTH_EAST: [f64; 2] = [1.0, -1.0];

/// A square loop from the origin in one direction, `perimeter` metres long.
fn square(perimeter: u32, direction: [f64; 2]) -> Value {
    trip(
        &rectangle(ORIGIN, perimeter / 4, direction),
        f64::from(perimeter),
    )
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

/// Answer the round trip requested with `seed`, exactly `times` times.
async fn answer_seed(server: &MockServer, seed: u32, response: ResponseTemplate, times: u64) {
    Mock::given(method("POST"))
        .and(body_partial_json(
            json!({ "options": { "round_trip": { "seed": seed } } }),
        ))
        .respond_with(response)
        .expect(times)
        .mount(server)
        .await;
}

/// Answer every round trip alike, exactly `times` times.
async fn provider(response: ResponseTemplate, times: u64) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(response)
        .expect(times)
        .mount(&server)
        .await;
    server
}

fn ok(body: Value) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(body)
}

fn distances(plan: &Value) -> Vec<f64> {
    plan["loops"]
        .as_array()
        .unwrap()
        .iter()
        .map(|route| route["distance_meters"].as_f64().unwrap())
        .collect()
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
async fn seeks_three_distinct_measured_walking_loops_through_the_pedestrian_provider() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/ors/v2/directions/foot-walking/geojson"))
        .and(body_partial_json(json!({
            "coordinates": [ORIGIN],
            "options": {
                "round_trip": { "length": 5000, "seed": 1 },
                "avoid_features": ["ferries"]
            }
        })))
        .respond_with(ok(square(5000, NORTH_EAST)))
        .expect(1)
        .mount(&server)
        .await;
    answer_seed(&server, 2, ok(square(5200, NORTH_WEST)), 1).await;
    answer_seed(&server, 3, ok(square(4900, SOUTH_WEST)), 1).await;
    // Three matches end the search; later seeds are never requested.
    for seed in 4..=6 {
        answer_seed(&server, seed, ok(square(5000, SOUTH_EAST)), 0).await;
    }

    let (status, body) = generate(app(demo(), Some(&server)).await, walk(5000)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["state"], "generated");
    let plan = &body["plan"];
    assert_eq!(plan["activity"], "walking");
    assert_eq!(
        plan["target"],
        json!({ "kind": "distance", "meters": 5000 })
    );
    assert_eq!(plan["tolerance_percent"], 10.0);
    assert_eq!(plan["sought"], 3);
    assert_eq!(plan["search"], "complete");
    // Closest to the target first; distances are measured from the drawn geometry.
    let measured = distances(plan);
    assert_eq!(measured.len(), 3);
    for (distance, expected) in measured.iter().zip([5000.0, 4900.0, 5200.0]) {
        assert!((distance - expected).abs() < 15.0, "{distance}");
    }
    for route in plan["loops"].as_array().unwrap() {
        let distance = route["distance_meters"].as_f64().unwrap();
        assert_eq!(route["deviation_meters"], distance - 5000.0);
        assert_eq!(route["within_tolerance"], true);
        // Steps are estimated from the unrounded measured distance.
        let steps = route["estimated_steps"].as_f64().unwrap();
        assert!((steps - distance / 0.75).abs() <= 1.5, "{steps}");
        let coordinates = route["geometry"]["coordinates"].as_array().unwrap();
        assert_eq!(coordinates[0], json!(ORIGIN));
        assert_eq!(coordinates.first(), coordinates.last());
    }
}

#[tokio::test]
async fn plans_running_loops_from_step_targets_avoiding_steps() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(body_partial_json(json!({
            "options": {
                "round_trip": { "length": 10000 },
                "avoid_features": ["ferries", "steps"]
            }
        })))
        .respond_with(ok(square(10_000, NORTH_EAST)))
        .expect(1..)
        .mount(&server)
        .await;

    let (status, body) = generate(
        app(demo(), Some(&server)).await,
        json!({
            "activity": "running",
            "target": { "kind": "steps", "steps": 10_000 },
            "step_length_cm": 100
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let plan = &body["plan"];
    assert_eq!(plan["activity"], "running");
    assert_eq!(
        plan["target"],
        json!({ "kind": "steps", "steps": 10_000, "meters": 10_000 })
    );
    assert_eq!(plan["step_length_cm"], 100);
    let route = &plan["loops"][0];
    assert_eq!(
        route["estimated_steps"].as_f64(),
        route["distance_meters"].as_f64()
    );
}

#[tokio::test]
async fn reports_fewer_distinct_matches_and_labels_deviations_without_padding() {
    let server = MockServer::start().await;
    answer_seed(&server, 1, ok(square(5000, NORTH_EAST)), 1).await;
    // The same loop again is not a second alternative.
    answer_seed(&server, 2, ok(square(5000, NORTH_EAST)), 1).await;
    // 5.8 km for a 5 km target is 16% long: kept, but never as a match.
    answer_seed(&server, 3, ok(square(5800, SOUTH_WEST)), 1).await;
    for seed in 4..=6 {
        answer_seed(
            &server,
            seed,
            ResponseTemplate::new(404).set_body_json(json!({ "error": { "code": 2009 } })),
            1,
        )
        .await;
    }

    let (_, body) = generate(app(demo(), Some(&server)).await, walk(5000)).await;
    let plan = &body["plan"];
    assert_eq!(plan["search"], "exhausted");
    let loops = plan["loops"].as_array().unwrap();
    assert_eq!(loops.len(), 2);
    assert_eq!(loops[0]["within_tolerance"], true);
    assert_eq!(loops[1]["within_tolerance"], false);
    let percent = loops[1]["deviation_percent"].as_f64().unwrap();
    assert!((percent - 16.0).abs() < 0.3, "{percent}");
}

#[tokio::test]
async fn replaces_a_near_miss_with_a_matching_loop_on_the_same_streets() {
    // The 5 km square plus a 400 m out-and-back spur: 5.8 km, 16% long.
    let square_ne = rectangle(ORIGIN, 1250, NORTH_EAST);
    let spur = rectangle(ORIGIN, 400, SOUTH_EAST);
    let mut near_miss = square_ne.clone();
    near_miss.extend(&spur[1..=8]);
    near_miss.extend(spur[1..8].iter().rev());
    near_miss.push(ORIGIN);
    let server = MockServer::start().await;
    answer_seed(&server, 1, ok(trip(&near_miss, 5800.0)), 1).await;
    answer_seed(&server, 2, ok(trip(&square_ne, 5000.0)), 1).await;
    for seed in 3..=6 {
        answer_seed(&server, seed, ok(trip(&square_ne, 5000.0)), 1).await;
    }

    let (_, body) = generate(app(demo(), Some(&server)).await, walk(5000)).await;
    let loops = body["plan"]["loops"].as_array().unwrap();
    assert_eq!(loops.len(), 1);
    assert_eq!(loops[0]["within_tolerance"], true);
}

#[tokio::test]
async fn keeps_searching_when_a_later_seed_cannot_place_its_waypoints() {
    let server = MockServer::start().await;
    answer_seed(&server, 1, ok(square(5000, NORTH_EAST)), 1).await;
    answer_seed(
        &server,
        2,
        ResponseTemplate::new(404).set_body_json(json!({ "error": { "code": 2010 } })),
        1,
    )
    .await;
    answer_seed(&server, 3, ok(square(5100, NORTH_WEST)), 1).await;
    answer_seed(&server, 4, ok(square(4900, SOUTH_WEST)), 1).await;
    for seed in 5..=6 {
        answer_seed(&server, seed, ok(square(5000, SOUTH_EAST)), 0).await;
    }

    let (_, body) = generate(app(demo(), Some(&server)).await, walk(5000)).await;
    assert_eq!(body["plan"]["search"], "complete");
    assert_eq!(distances(&body["plan"]).len(), 3);
}

#[tokio::test]
async fn matches_the_tolerance_verdict_to_the_displayed_percentage() {
    // Southbound squares measure just over 10% and 10.05% long respectively.
    // Only the first rounds to the displayed 10.0% tolerance boundary.
    for (perimeter, percent, matches) in [(5500, 10.0, true), (5504, 10.1, false)] {
        let server = provider(ok(square(perimeter, SOUTH_EAST)), MAX_ATTEMPTS).await;
        let (_, body) = generate(app(demo(), Some(&server)).await, walk(5000)).await;
        let route = &body["plan"]["loops"][0];
        assert_eq!(route["deviation_percent"], percent);
        assert_eq!(route["within_tolerance"], matches);
    }
}

#[tokio::test]
async fn bounds_a_queued_search_and_reports_provider_unavailability() {
    let server = MockServer::start().await;
    // Three slow, individually valid responses occupy the provider for 27 s.
    for (seed, direction) in [(1, NORTH_EAST), (2, NORTH_WEST), (3, SOUTH_WEST)] {
        answer_seed(
            &server,
            seed,
            ok(square(5000, direction)).set_delay(Duration::from_secs(9)),
            1,
        )
        .await;
    }
    let app = app(demo(), Some(&server)).await;
    let first = tokio::spawn(generate(app.clone(), walk(5000)));
    // Start the different-target request only once the provider has received the first.
    tokio::time::timeout(Duration::from_secs(5), async {
        while server.received_requests().await.unwrap().is_empty() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the first search must reach the provider");

    let queued = tokio::time::timeout(Duration::from_secs(22), generate(app, walk(10_000))).await;
    let (_, first) = first.await.unwrap();
    assert_eq!(first["plan"]["search"], "complete");
    let (status, body) = queued.expect("a queued search must answer within its own time budget");
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        json!({ "state": "unavailable", "reason": "provider-unavailable" })
    );
}

#[tokio::test]
async fn corrects_later_requests_for_a_provider_that_overshoots() {
    let server = MockServer::start().await;
    // 6.25 km for 5 km overshoots by 25%, so the next request asks for 4 km.
    answer_seed(&server, 1, ok(square(6250, NORTH_EAST)), 1).await;
    Mock::given(method("POST"))
        .and(|request: &Request| {
            let body: Value = request.body_json().unwrap();
            let round_trip = &body["options"]["round_trip"];
            round_trip["seed"] == 2
                && (3990..=4010).contains(&round_trip["length"].as_u64().unwrap())
        })
        .respond_with(ok(square(5000, SOUTH_WEST)))
        .expect(1)
        .mount(&server)
        .await;
    for seed in 3..=6 {
        answer_seed(&server, seed, ok(square(5000, SOUTH_WEST)), 1).await;
    }

    let (_, body) = generate(app(demo(), Some(&server)).await, walk(5000)).await;
    let measured = distances(&body["plan"]);
    assert_eq!(measured.len(), 2);
    assert!((measured[0] - 5000.0).abs() < 15.0, "{measured:?}");
}

#[tokio::test]
async fn reuses_completed_searches_but_retries_interrupted_ones() {
    let server = provider(ok(square(5000, NORTH_EAST)), MAX_ATTEMPTS).await;
    let app = app(demo(), Some(&server)).await;
    let (_, first) = generate(app.clone(), walk(5000)).await;
    let (_, second) = generate(app.clone(), walk(5000)).await;
    assert_eq!(first["plan"]["search"], "exhausted");
    assert_eq!(first, second);
    // The same distance as steps reuses the loops and estimates steps with its own length.
    let (_, steps) = generate(
        app,
        json!({
            "activity": "walking",
            "target": { "kind": "steps", "steps": 5000 },
            "step_length_cm": 100
        }),
    )
    .await;
    assert_eq!(
        steps["plan"]["loops"][0]["geometry"],
        first["plan"]["loops"][0]["geometry"]
    );
    assert_eq!(
        steps["plan"]["loops"][0]["estimated_steps"].as_f64(),
        first["plan"]["loops"][0]["distance_meters"].as_f64()
    );

    let server = MockServer::start().await;
    answer_seed(&server, 1, ok(square(5000, NORTH_EAST)), 2).await;
    answer_seed(&server, 2, ResponseTemplate::new(502), 2).await;
    let app = self::app(demo(), Some(&server)).await;
    for _ in 0..2 {
        let (_, body) = generate(app.clone(), walk(5000)).await;
        assert_eq!(body["plan"]["search"], "interrupted");
        assert_eq!(distances(&body["plan"]).len(), 1);
    }
}

#[tokio::test]
async fn rejects_loops_that_do_not_start_at_the_origin_close_or_fit_the_target() {
    let far_start = [ORIGIN[0] + 0.01, ORIGIN[1]];
    let mut open = rectangle(ORIGIN, 1250, NORTH_EAST);
    open.truncate(open.len() - 5);
    for (coordinates, summary) in [
        (rectangle(far_start, 1250, NORTH_EAST), 5000.0),
        (open.clone(), 4750.0),
        (rectangle(ORIGIN, 500, NORTH_EAST), 2000.0),
    ] {
        let server = provider(ok(trip(&coordinates, summary)), MAX_ATTEMPTS).await;
        let (_, body) = generate(app(demo(), Some(&server)).await, walk(5000)).await;
        assert_eq!(
            body,
            json!({ "state": "unavailable", "reason": "unsuitable-route" })
        );
    }
}

#[tokio::test]
async fn rejects_invalid_provider_responses_without_inventing_a_route() {
    let valid = square(5000, NORTH_EAST);
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
        // An invalid answer ends the search rather than spending more attempts.
        let server = provider(ok(response), 1).await;
        let (_, body) = generate(app(demo(), Some(&server)).await, walk(5000)).await;
        assert_eq!(
            body,
            json!({ "state": "unavailable", "reason": "provider-unavailable" })
        );
    }
}

#[tokio::test]
async fn reports_no_route_refusal_and_outage_outcomes_without_upstream_text() {
    for (response, attempts, reason) in [
        (
            ResponseTemplate::new(404).set_body_json(
                json!({ "error": { "code": 2009, "message": "synthetic-upstream-detail" } }),
            ),
            MAX_ATTEMPTS,
            "no-route",
        ),
        // No walkable way near the origin: other seeds cannot help.
        (
            ResponseTemplate::new(404).set_body_json(json!({ "error": { "code": 2010 } })),
            1,
            "no-route",
        ),
        (
            ResponseTemplate::new(400).set_body_json(json!({ "error": { "code": 2003 } })),
            1,
            "provider-unavailable",
        ),
        (
            ResponseTemplate::new(502).set_body_string("synthetic-upstream-detail"),
            1,
            "provider-unavailable",
        ),
        // A rate-limited or over-quota instance is a provider failure (ADR 0008).
        (
            ResponseTemplate::new(429).set_body_json(json!({ "error": { "code": 2099 } })),
            1,
            "provider-unavailable",
        ),
    ] {
        let server = provider(response, attempts).await;
        let (status, body) = generate(app(demo(), Some(&server)).await, walk(5000)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, json!({ "state": "unavailable", "reason": reason }));
    }
}

#[tokio::test]
async fn rejects_invalid_requests_before_contacting_the_provider() {
    let server = provider(ResponseTemplate::new(500), 0).await;
    let app = app(demo(), Some(&server)).await;
    for (body, message) in [
        (
            json!({ "activity": "cycling", "target": { "kind": "distance", "meters": 5000 }, "step_length_cm": 75 }),
            "Unsupported activity; choose walking or running",
        ),
        (
            walk(999),
            "Target distance must be between 1 km and 25 km, in whole metres",
        ),
        (
            json!({ "activity": "walking", "target": { "kind": "distance", "meters": 5000.5 }, "step_length_cm": 75 }),
            "Target distance must be between 1 km and 25 km, in whole metres",
        ),
        (
            json!({ "activity": "walking", "target": { "kind": "steps", "steps": 40_000 }, "step_length_cm": 75 }),
            "Step target must be a whole number of steps covering between 1 km and 25 km",
        ),
        (
            json!({ "activity": "walking", "target": { "kind": "steps", "steps": 10_000 }, "step_length_cm": 20 }),
            "Step length must be a whole number of centimetres between 40 and 150",
        ),
        (
            json!({ "activity": "walking", "target": { "kind": "pace", "meters": 5000 }, "step_length_cm": 75 }),
            "Invalid loop request",
        ),
        (
            json!({ "activity": "walking", "target_meters": 5000 }),
            "Invalid loop request",
        ),
    ] {
        let (status, response) = generate(app.clone(), body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(response["error"], message);
    }
}
