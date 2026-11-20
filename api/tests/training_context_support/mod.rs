#![allow(dead_code)]

use crate::common;
use crate::common::mirror::Mirror;
use crate::common::mirror::Window;
use axum::Router;
use axum::http::StatusCode;
use chrono::Duration;
use openhome_api::services::training::CalendarDay;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;

pub(super) const API_KEY: &str = "test-api-key";

pub(super) const SOURCE: &str = "81906e30-187d-4546-8f8a-9949b82d62fa";

pub(super) const IMPORTED: &str = "my-whoop";

pub(super) const COMPUTED: &str = "my-whoop-noop";

pub(super) fn day(text: &str) -> CalendarDay {
    CalendarDay::parse(text).unwrap()
}

pub(super) fn at(text: &str, offset: Duration) -> i64 {
    (day(text).start() + offset).timestamp()
}

pub(super) fn daily(day: &str, fields: Value) -> Value {
    let mut data = json!({
        "totalSleepMin": null, "efficiency": null, "deepMin": null, "remMin": null,
        "lightMin": null, "disturbances": null, "restingHr": null, "avgHrv": null,
        "recovery": null, "strain": null, "exerciseCount": null, "spo2Pct": null,
        "skinTempDevC": null, "respRateBpm": null, "steps": null, "activeKcalEst": null,
        "spo2Red": null, "spo2Ir": null,
    });
    let Value::Object(fields) = fields else {
        panic!("fields must be a JSON object");
    };
    for (field, value) in fields {
        data[&field] = value;
    }
    json!({"type": "record", "key": {"day": day}, "data": data})
}

pub(super) fn noop_workout(
    start_ts: i64,
    end_ts: i64,
    sport: &str,
    source: &str,
    fields: Value,
) -> Value {
    let mut data = json!({
        "endTs": end_ts, "source": source, "durationS": null, "energyKcal": null, "avgHr": null,
        "maxHr": null, "strain": null, "distanceM": null, "zonesJSON": null, "notes": null,
        "routePolyline": null, "steps": null,
    });
    let Value::Object(fields) = fields else {
        panic!("fields must be a JSON object");
    };
    for (field, value) in fields {
        data[&field] = value;
    }
    json!({"type": "record", "key": {"startTs": start_ts, "sport": sport}, "data": data})
}

pub(super) async fn mirror(
    from: &'static str,
    to: &'static str,
    imported: &[Value],
    computed: &[Value],
    workouts: &[Value],
    pushed: &str,
) -> Mirror {
    let mut mirror = Mirror::empty().await;
    let days = Window::Days(from, to);
    let from_ts = day(from).start().timestamp();
    let to_ts = day(to).start().timestamp();
    for (device, rows, device_workouts) in
        [(IMPORTED, imported, workouts), (COMPUTED, computed, &[])]
    {
        mirror.push(SOURCE, device, days, rows, pushed).await;
        mirror
            .push(SOURCE, device, Window::Starts(from_ts, to_ts), &[], pushed)
            .await;
        mirror
            .push(
                SOURCE,
                device,
                Window::WorkoutStarts(from_ts, to_ts),
                device_workouts,
                pushed,
            )
            .await;
    }
    mirror
}

pub(super) async fn context(
    app: &Router,
    from: &str,
    to: &str,
    key: Option<&str>,
) -> (StatusCode, Value) {
    common::send_request(
        app.clone(),
        &format!("/api/training/context/{from}/{to}"),
        key,
    )
    .await
}

pub(super) async fn context_ok(app: &Router, from: &str, to: &str) -> Value {
    let (status, body) = context(app, from, to, Some(API_KEY)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body
}

pub(super) fn sync(recovery: &str, workouts: &str, coverage: &str) -> Value {
    json!({
        "recovery": {
            "freshness": recovery,
            "coverage": {"daily_metrics": coverage, "sleep_sessions": coverage},
        },
        "workouts": {"freshness": workouts, "coverage": {"workouts": coverage}},
    })
}
