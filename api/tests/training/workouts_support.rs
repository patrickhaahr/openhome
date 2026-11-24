use crate::mirror::Mirror;
use crate::mirror::Window;
use crate::mirror::noop_workout;
use crate::send_request::send_request;
use crate::send_request_with_method::send_request_with_method;
use axum::Router;
use axum::http::Method;
use axum::http::StatusCode;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;

pub(crate) const API_KEY: &str = "test-api-key";

pub(crate) const SOURCE: &str = "81906e30-187d-4546-8f8a-9949b82d62fa";

pub(crate) const IMPORTED: &str = "my-whoop";

pub(crate) const COMPUTED: &str = "my-whoop-noop";

pub(crate) const REAL_PUSH_AT: &str = "2026-09-26T01:51:38.318Z";

pub(crate) const REAL_STARTS: Window = Window::WorkoutStarts(1_789_250_400, 1_790_460_000);

pub(crate) const PULL_UP: i64 = 11;

pub(crate) fn real_noop_workout() -> Value {
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

pub(crate) async fn mirror_with(
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

pub(crate) async fn app(mirror: &Mirror) -> Router {
    crate::mirror::test_app_with_noop_db(mirror.db.clone()).await
}

pub(crate) async fn log_workout(app: &Router, workout: Value) -> i64 {
    let (status, body) = send_request_with_method(
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

pub(crate) async fn workouts(
    app: &Router,
    day: &str,
    api_key: Option<&str>,
) -> (StatusCode, Value) {
    send_request(
        app.clone(),
        &format!("/api/training/days/{day}/workouts"),
        api_key,
    )
    .await
}

pub(crate) async fn workouts_on(app: &Router, day: &str) -> Value {
    let (status, body) = workouts(app, day, Some(API_KEY)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body
}
