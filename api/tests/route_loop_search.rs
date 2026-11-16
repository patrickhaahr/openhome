//! Route loop search: measured loops, targets and tolerance.

mod common;
mod route_loop_support;

use axum::http::Method;
use axum::http::StatusCode;
use common::send_request_with_method;
use route_loop_support::{
    MAX_ATTEMPTS, NORTH_EAST, NORTH_WEST, ORIGIN, SOUTH_EAST, SOUTH_WEST, answer_seed, app, demo,
    distances, generate, ok, provider, rectangle, square, trip, walk,
};
use serde_json::json;
use std::time::Duration;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::body_partial_json;
use wiremock::matchers::method;
use wiremock::matchers::path;

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
