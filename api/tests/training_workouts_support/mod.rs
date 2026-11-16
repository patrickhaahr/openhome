//! NOOP mirror fixtures shared by the Training Day Workouts test files.

#![allow(dead_code)]

use crate::common;
use crate::common::mirror::Mirror;
use crate::common::mirror::Window;
use crate::common::mirror::noop_workout;
use axum::Router;
use axum::http::Method;
use axum::http::StatusCode;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;

pub(super) const API_KEY: &str = "test-api-key";

/// The production installation and NOOP's strap namespaces.
pub(super) const SOURCE: &str = "81906e30-187d-4546-8f8a-9949b82d62fa";

pub(super) const IMPORTED: &str = "my-whoop";

pub(super) const COMPUTED: &str = "my-whoop-noop";

/// The production mirror's latest push: 03:51 local time on 2026-09-26.
pub(super) const REAL_PUSH_AT: &str = "2026-09-26T01:51:38.318Z";

/// NOOP's real 14-day workout window ending with 2026-09-26 (local midnights).
pub(super) const REAL_STARTS: Window = Window::WorkoutStarts(1_789_250_400, 1_790_460_000);

/// Seeded exercise library ids.
pub(super) const PULL_UP: i64 = 11;

/// The real NOOP Workout: Calisthenics timed manually in NOOP, 02:12:38 → 03:30:11 on 2026-09-26.
pub(super) fn real_noop_workout() -> Value {
    noop_workout(
        1_790_381_558,
        1_790_386_211,
        "Calisthenics",
        "manual",
        json!({
            "durationS": 4653.247, "energyKcal": 400.8306717632, "avgHr": 81.0, "maxHr": 134.0,
            "strain": 24.51,
        }),
    )
}

/// Both namespaces pushed `workout` windows with the given rows, accepted at `accepted_at`.
pub(super) async fn mirror_with(
    imported: &[Value],
    computed: &[Value],
    accepted_at: &str,
) -> Mirror {
    let mut mirror = Mirror::empty().await;
    mirror
        .push(SOURCE, IMPORTED, REAL_STARTS, imported, accepted_at)
        .await;
    mirror
        .push(SOURCE, COMPUTED, REAL_STARTS, computed, accepted_at)
        .await;
    mirror
}

/// The API over `mirror` with an empty training log.
pub(super) async fn app(mirror: &Mirror) -> Router {
    common::test_app_with_noop_db(mirror.db.clone()).await
}

/// Logs a Workout through the fitness API and returns its id.
pub(super) async fn log_workout(app: &Router, workout: Value) -> i64 {
    let (status, body) = common::send_request_with_method(
        app.clone(),
        "/api/workouts",
        Method::POST,
        Some(workout),
        Some(API_KEY),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body["id"].as_i64().unwrap()
}

pub(super) async fn workouts(
    app: &Router,
    day: &str,
    api_key: Option<&str>,
) -> (StatusCode, Value) {
    common::send_request(
        app.clone(),
        &format!("/api/training/days/{day}/workouts"),
        api_key,
    )
    .await
}

pub(super) async fn workouts_on(app: &Router, day: &str) -> Value {
    let (status, body) = workouts(app, day, Some(API_KEY)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body
}
