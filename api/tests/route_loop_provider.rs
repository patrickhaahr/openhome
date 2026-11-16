//! Route loop provider handling: overshoot, retries, invalid answers and the time budget.

mod common;
mod route_loop_support;

use axum::http::StatusCode;
use route_loop_support::{
    MAX_ATTEMPTS, NORTH_EAST, NORTH_WEST, ORIGIN, SOUTH_EAST, SOUTH_WEST, answer_seed, app, demo,
    distances, generate, ok, provider, rectangle, square, trip, walk,
};
use serde_json::Value;
use serde_json::json;
use std::time::Duration;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::Request;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;

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

#[tokio::test]
async fn a_request_queued_behind_the_same_target_reuses_the_first_answer() {
    let server = MockServer::start().await;
    answer_seed(&server, 1, ok(square(5000, NORTH_EAST)), 1).await;
    answer_seed(&server, 2, ok(square(5200, NORTH_WEST)), 1).await;
    answer_seed(&server, 3, ok(square(5050, SOUTH_WEST)), 1).await;
    for seed in 4..=6 {
        answer_seed(&server, seed, ok(square(5000, SOUTH_EAST)), 0).await;
    }

    let app = app(demo(), Some(&server)).await;
    let ((status_a, first), (status_b, second)) =
        tokio::join!(generate(app.clone(), walk(5000)), generate(app, walk(5000)));
    assert_eq!(status_a, StatusCode::OK);
    assert_eq!(status_b, StatusCode::OK);
    assert_eq!(first, second, "both callers must receive the same plan");
    assert_eq!(first["plan"]["search"], "complete");
    // The identical target is searched once; the queued caller reuses its answer.
    assert_eq!(server.received_requests().await.unwrap().len(), 3);
}

#[tokio::test]
async fn calibrates_the_next_request_from_an_unsuitable_loop_overshoot() {
    let server = MockServer::start().await;
    // An unsuitable 6.25 km loop starting 780 m away still measures a 25% overshoot.
    let far_start = [ORIGIN[0] + 0.01, ORIGIN[1]];
    answer_seed(
        &server,
        1,
        ok(trip(&rectangle(far_start, 1_562, NORTH_EAST), 6250.0)),
        1,
    )
    .await;
    // Only an attempt at the calibrated 4 km length is answered; seeds 5 and 6 are never spent.
    for (seed, direction) in [(2, NORTH_WEST), (3, SOUTH_WEST), (4, SOUTH_EAST)] {
        Mock::given(method("POST"))
            .and(move |request: &Request| {
                let body: Value = request.body_json().unwrap();
                let round_trip = &body["options"]["round_trip"];
                round_trip["seed"] == seed
                    && (3995.0..=4005.0).contains(&round_trip["length"].as_f64().unwrap())
            })
            .respond_with(ok(square(5000, direction)))
            .expect(1)
            .mount(&server)
            .await;
    }
    for seed in 5..=6 {
        answer_seed(&server, seed, ok(square(5000, NORTH_EAST)), 0).await;
    }

    let (_, body) = generate(app(demo(), Some(&server)).await, walk(5000)).await;
    let plan = &body["plan"];
    assert_eq!(plan["search"], "complete");
    assert_eq!(distances(plan).len(), 3);
}

#[tokio::test]
async fn interrupts_with_partial_loops_when_no_attempt_may_start_after_the_time_budget() {
    let server = MockServer::start().await;
    // Three slow attempts spend the 20 s budget; the third retraces the first, so the search
    // is still incomplete when the budget cuts before a fourth attempt.
    for (seed, direction) in [(1, NORTH_EAST), (2, NORTH_WEST), (3, NORTH_EAST)] {
        answer_seed(
            &server,
            seed,
            ok(square(5000, direction)).set_delay(Duration::from_secs(8)),
            1,
        )
        .await;
    }
    for seed in 4..=6 {
        answer_seed(&server, seed, ok(square(5000, SOUTH_EAST)), 0).await;
    }

    let (_, body) = generate(app(demo(), Some(&server)).await, walk(5000)).await;
    let plan = &body["plan"];
    assert_eq!(plan["search"], "interrupted");
    let measured = distances(plan);
    // Both distinct loops are still shown next to the interrupted search.
    assert_eq!(measured.len(), 2);
    for distance in &measured {
        assert!((distance - 5000.0).abs() < 15.0, "{distance}");
    }
}

#[tokio::test]
async fn partial_loops_survive_invalid_geometry_after_a_retained_loop() {
    let server = MockServer::start().await;
    answer_seed(&server, 1, ok(square(5000, NORTH_EAST)), 1).await;
    let valid = square(5000, NORTH_WEST);
    let mut two_features = valid.clone();
    two_features["features"] = json!([valid["features"][0], valid["features"][0]]);
    answer_seed(&server, 2, ok(two_features), 1).await;
    for seed in 3..=6 {
        answer_seed(&server, seed, ok(square(5000, SOUTH_WEST)), 0).await;
    }

    // The invalid second answer ends the search rather than spending more attempts, but
    // the first retained loop remains available next to the interrupted search.
    let (_, body) = generate(app(demo(), Some(&server)).await, walk(5000)).await;
    let plan = &body["plan"];
    assert_eq!(plan["search"], "interrupted");
    let measured = distances(plan);
    assert_eq!(measured.len(), 1);
    assert!((measured[0] - 5000.0).abs() < 15.0, "{measured:?}");
}
