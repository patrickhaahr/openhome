//! App, request and fixture helpers shared by the NOOP Workout Detail test files.

#![allow(dead_code)]

use crate::common;
use crate::common::mirror::Mirror;
use crate::common::mirror::noop_workout;
use crate::common::reference_run::IMPORTED;
use crate::common::reference_run::PUSHED_AT;
use crate::common::reference_run::SOURCE;
use crate::common::reference_run::WORKOUT_WINDOW;
use axum::Router;
use axum::http::Method;
use axum::http::StatusCode;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;

pub(super) const API_KEY: &str = "test-api-key";

/// The API over `mirror` with an empty training log.
pub(super) async fn app(mirror: &Mirror) -> Router {
    common::test_app_with_noop_db(mirror.db.clone()).await
}

pub(super) async fn detail(
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

pub(super) async fn detail_of(app: &Router, source: &str, start: &str) -> Value {
    let (status, body) = detail(app, source, start, Some(API_KEY)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body
}

/// Sets the Profile's Max Heart Rate, or clears it with `None`.
pub(super) async fn set_hr_max(app: &Router, hr_max_bpm: Option<i64>) {
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

/// Workouts on the Training Day `day`.
pub(super) async fn workouts_on(app: &Router, day: &str) -> Value {
    let (status, body) = common::send_request(
        app.clone(),
        &format!("/api/training/days/{day}/workouts"),
        Some(API_KEY),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body
}

/// Asserts `actual` is a number within 1e-9 of `expected`.
pub(super) fn assert_close(actual: &Value, expected: f64) {
    let actual = actual
        .as_f64()
        .unwrap_or_else(|| panic!("{actual} is not a number"));
    assert!((actual - expected).abs() < 1e-9, "{actual} != {expected}");
}

/// A mirror where only the strap's imported namespace pushed `rows` in the workout window.
pub(super) async fn mirror_with(rows: &[Value]) -> Mirror {
    let mut mirror = Mirror::empty().await;
    mirror
        .push(SOURCE, IMPORTED, WORKOUT_WINDOW, rows, PUSHED_AT)
        .await;
    mirror
}

/// The API over a mirror holding one strap-recorded Running workout from 07:00 on 2026-09-26
/// lasting `duration_s`, with strap samples at `(offset_s, bpm)` from its start, and the Profile's
/// Max Heart Rate set to `hr_max_bpm`.
pub(super) async fn app_with_run(
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
