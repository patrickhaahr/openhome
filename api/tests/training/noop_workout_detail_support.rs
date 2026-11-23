use crate::common;
use crate::mirror::Mirror;
use crate::mirror::noop_workout;
use crate::reference_run::IMPORTED;
use crate::reference_run::PUSHED_AT;
use crate::reference_run::SOURCE;
use crate::reference_run::WORKOUT_WINDOW;
use axum::Router;
use axum::http::Method;
use axum::http::StatusCode;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;

pub(crate) const API_KEY: &str = "test-api-key";

pub(crate) async fn app(mirror: &Mirror) -> Router {
    common::test_app_with_noop_db(mirror.db.clone()).await
}

pub(crate) async fn detail(
    app: &Router,
    source: &str,
    start: &str,
    api_key: Option<&str>,
) -> (StatusCode, Value) {
    common::send_request(
        app.clone(),
        &format!("/api/training/noop-workouts/{source}/{start}"),
        api_key,
    )
    .await
}

pub(crate) async fn detail_of(app: &Router, source: &str, start: &str) -> Value {
    let (status, body) = detail(app, source, start, Some(API_KEY)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body
}

pub(crate) async fn set_hr_max(app: &Router, hr_max_bpm: Option<i64>) {
    let (status, body) = common::send_request_with_method(
        app.clone(),
        "/api/profile",
        Method::PATCH,
        Some(json!({"hr_max_bpm": hr_max_bpm})),
        Some(API_KEY),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
}

pub(crate) async fn workouts_on(app: &Router, day: &str) -> Value {
    let (status, body) = common::send_request(
        app.clone(),
        &format!("/api/training/days/{day}/workouts"),
        Some(API_KEY),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body
}

pub(crate) fn assert_close(actual: &Value, expected: f64) {
    let actual = actual
        .as_f64()
        .unwrap_or_else(|| panic!("{actual} is not a number"));
    assert!((actual - expected).abs() < 1e-9, "{actual} != {expected}");
}

pub(crate) async fn mirror_with(rows: &[Value]) -> Mirror {
    let mut mirror = Mirror::empty().await;
    mirror
        .push(SOURCE, IMPORTED, WORKOUT_WINDOW, rows, PUSHED_AT)
        .await;
    mirror
}

pub(crate) async fn app_with_run(
    duration_s: i64,
    samples: &[(i64, i64)],
    hr_max_bpm: Option<i64>,
) -> Router {
    let start = 1_790_398_800;
    let mut mirror = mirror_with(&[noop_workout(
        start,
        start + duration_s,
        "Running",
        "manual",
        json!({}),
    )])
    .await;
    let samples: Vec<_> = samples
        .iter()
        .map(|&(offset, bpm)| (start + offset, bpm))
        .collect();
    mirror
        .push_hr_samples(SOURCE, IMPORTED, &samples, PUSHED_AT)
        .await;
    let app = app(&mirror).await;
    set_hr_max(&app, hr_max_bpm).await;
    app
}
