//! Receiver app, request and batch builders shared by the NOOP push test files.

#![allow(dead_code)]

use crate::common;
use axum::Router;
use axum::body::Body;
use axum::http::Method;
use axum::http::Request;
use axum::http::StatusCode;
use openhome_api::routes::noop_push::PUSH_PATH;
use openhome_api::routes::noop_push::PushState;
use openhome_api::routes::noop_push::router as push_router;
use openhome_api::services::noop_push;
use openhome_api::services::noop_push::PushToken;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use sqlx::SqlitePool;
use sqlx::sqlite::SqlitePoolOptions;
use tower::ServiceExt;

pub(super) const PUSH_TOKEN: &str = "test-push-token";

pub(super) const API_KEY: &str = "test-api-key";

pub(super) async fn memory_push_db() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    noop_push::initialize(&pool).await.unwrap();
    pool
}

/// Composes the app the way `main` does: API routes behind the API-key layer (applied by
/// `common`), the push router merged afterwards with its own token.
pub(super) async fn app_with_push_db(push_db: SqlitePool) -> Router {
    let (api, _) = common::test_app_with_db().await;
    let push_state = PushState {
        db: push_db,
        token: PushToken::new(PUSH_TOKEN.to_string()).unwrap(),
    };

    api.merge(push_router(push_state))
}

pub(super) async fn app() -> Router {
    app_with_push_db(memory_push_db().await).await
}

pub(super) struct Reply {
    pub(super) status: StatusCode,
    pub(super) headers: http::HeaderMap,
    pub(super) raw: Vec<u8>,
    pub(super) json: Value,
}

pub(super) fn bearer(token: &str) -> String {
    format!("Bearer {token}")
}

// ---------------------------------------------------------------------------------------------
// POST: append batch ingest
// ---------------------------------------------------------------------------------------------

pub(super) const SOURCE_A: &str = "3a3486dd-5030-4e17-a00d-a781399890f9";

pub(super) const SOURCE_B: &str = "7c9e6679-7425-40de-944b-e07fc1f90ae7";

pub(super) const DEVICE: &str = "strap-local-id";

pub(super) const BATCH_1: &str = "e835f32f-60e7-4c93-90a0-51eb6830119a";

pub(super) const BATCH_2: &str = "0f8fad5b-d9cb-469f-a165-70867728950e";

pub(super) const START_SHA: &str =
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

pub(super) const END_SHA: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

pub(super) fn push_app(db: SqlitePool) -> Router {
    push_router(PushState {
        db,
        token: PushToken::new(PUSH_TOKEN.to_string()).unwrap(),
    })
}

pub(super) fn cursor(row_id: i64, key_sha256: &str) -> Value {
    json!({"rowId": row_id, "keySha256": key_sha256})
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "JSON builders take `json!` literals by value, as `json!` itself does"
)]
pub(super) fn record(key: Value, data: Value) -> Value {
    json!({"type": "record", "key": key, "data": data})
}

pub(super) fn hr(ts: i64, bpm: i64) -> Value {
    record(json!({"ts": ts}), json!({"bpm": bpm}))
}

/// A valid append header for `count` records at insertion positions 101..=100+count.
pub(super) fn header(stream: &str, batch_id: &str, count: usize) -> Value {
    json!({
        "type": "batch",
        "protocolVersion": "1.0",
        "batchId": batch_id,
        "sourceId": SOURCE_A,
        "deviceId": DEVICE,
        "stream": stream,
        "delivery": "append",
        "recordCount": count,
        "startCursor": cursor(100, START_SHA),
        "endCursor": cursor(100 + count as i64, END_SHA),
    })
}

pub(super) fn ndjson(header: &Value, records: &[Value]) -> Vec<u8> {
    let mut entity = String::new();
    for line in std::iter::once(header).chain(records) {
        entity.push_str(&line.to_string());
        entity.push('\n');
    }
    entity.into_bytes()
}

pub(super) fn hr_batch(batch_id: &str, records: &[Value]) -> Vec<u8> {
    ndjson(&header("hrSample", batch_id, records.len()), records)
}

pub(super) fn gzip(bytes: &[u8]) -> Vec<u8> {
    use std::io::Write;
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(bytes).unwrap();
    encoder.finish().unwrap()
}

pub(super) async fn send(app: Router, headers: &[(&str, &str)], body: Vec<u8>) -> Reply {
    let mut builder = Request::builder().method(Method::POST).uri(PUSH_PATH);
    for (name, value) in headers {
        builder = builder.header(*name, *value);
    }
    let response = app
        .oneshot(builder.body(Body::from(body)).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let raw = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap()
        .to_vec();
    let json = serde_json::from_slice(&raw).unwrap_or(Value::Null);
    Reply {
        status,
        headers,
        raw,
        json,
    }
}

/// POSTs a decoded entity, gzip-encoded when `gzipped`, the way the Android client does.
pub(super) async fn post(db: &SqlitePool, entity: &[u8], gzipped: bool) -> Reply {
    let auth = bearer(PUSH_TOKEN);
    let mut headers = vec![
        ("authorization", auth.as_str()),
        ("content-type", "application/x-ndjson; charset=utf-8"),
        ("accept", "application/json"),
    ];
    let body = if gzipped {
        headers.push(("content-encoding", "gzip"));
        gzip(entity)
    } else {
        entity.to_vec()
    };
    send(push_app(db.clone()), &headers, body).await
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "JSON builders take `json!` literals by value, as `json!` itself does"
)]
pub(super) fn expected_ack(stream: &str, batch_id: &str, end_cursor: Value, rows: usize) -> Value {
    json!({
        "protocolVersion": "1.0",
        "batchId": batch_id,
        "stream": stream,
        "deviceId": DEVICE,
        "endCursor": end_cursor,
        "acceptedRows": rows,
        "status": "accepted",
    })
}

#[track_caller]
pub(super) fn assert_error(reply: &Reply, status: StatusCode, code: &str) {
    assert_eq!(
        reply.status,
        status,
        "{}",
        String::from_utf8_lossy(&reply.raw)
    );
    assert_eq!(
        reply.json,
        json!({"type": "error", "protocolVersion": "1.0", "code": code})
    );
    assert!(reply.raw.len() < 256);
}

pub(super) async fn count(db: &SqlitePool, sql: &str) -> i64 {
    sqlx::query_scalar(sql).fetch_one(db).await.unwrap()
}

pub(super) async fn rows(db: &SqlitePool, sql: &str) -> Vec<Value> {
    let rows: Vec<String> = sqlx::query_scalar(sql).fetch_all(db).await.unwrap();
    rows.iter()
        .map(|row| serde_json::from_str(row).unwrap())
        .collect()
}

// ---------------------------------------------------------------------------------------------
// POST: replace-window delivery for the mutable streams
// ---------------------------------------------------------------------------------------------

/// A distinct canonical UUID per `n` (batch and replacement IDs).
pub(super) fn id(n: u64) -> String {
    format!("00000000-0000-4000-8000-{n:012x}")
}

pub(super) fn day(index: i64) -> String {
    format!("2026-08-{index:02}")
}

/// Local-midnight-like Unix seconds for day `index` (UTC here; the receiver never interprets it).
pub(super) const fn ts(index: i64) -> i64 {
    1_754_006_400 + index * 86_400
}

pub(super) fn day_window(replacement: &str, start: i64, end: i64, part: u32, parts: u32) -> Value {
    json!({
        "replacementId": replacement,
        "selector": "day",
        "startInclusive": day(start),
        "endExclusive": day(end),
        "part": part,
        "parts": parts,
    })
}

pub(super) fn ts_window(replacement: &str, start: i64, end: i64, part: u32, parts: u32) -> Value {
    json!({
        "replacementId": replacement,
        "selector": "startTs",
        "startInclusive": ts(start),
        "endExclusive": ts(end),
        "part": part,
        "parts": parts,
    })
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "JSON builders take `json!` literals by value, as `json!` itself does"
)]
pub(super) fn replace_header(stream: &str, batch_id: &str, count: usize, window: Value) -> Value {
    json!({
        "type": "batch",
        "protocolVersion": "1.0",
        "batchId": batch_id,
        "sourceId": SOURCE_A,
        "deviceId": DEVICE,
        "stream": stream,
        "delivery": "replace_window",
        "recordCount": count,
        "startCursor": null,
        "endCursor": null,
        "window": window,
    })
}

pub(super) fn replace_part(
    stream: &str,
    batch_id: &str,
    window: Value,
    records: &[Value],
) -> Vec<u8> {
    ndjson(
        &replace_header(stream, batch_id, records.len(), window),
        records,
    )
}

pub(super) fn replace_ack(stream: &str, batch_id: &str, rows: usize) -> Value {
    expected_ack(stream, batch_id, Value::Null, rows)
}

#[track_caller]
pub(super) fn assert_ok(reply: &Reply, expected: &Value) {
    assert_eq!(
        reply.status,
        StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&reply.raw)
    );
    assert_eq!(&reply.json, expected);
}

pub(super) fn daily(index: i64, recovery: f64) -> Value {
    record(
        json!({"day": day(index)}),
        json!({
            "totalSleepMin": null, "efficiency": null, "deepMin": null, "remMin": null,
            "lightMin": null, "disturbances": null, "restingHr": null, "avgHrv": null,
            "recovery": recovery, "strain": null, "exerciseCount": null, "spo2Pct": null,
            "skinTempDevC": null, "respRateBpm": null, "steps": null, "activeKcalEst": null,
            "spo2Red": null, "spo2Ir": null,
        }),
    )
}

pub(super) fn sleep(index: i64, end_offset: i64) -> Value {
    record(
        json!({"startTs": ts(index)}),
        json!({
            "endTs": ts(index) + end_offset, "efficiency": null, "restingHr": null,
            "avgHrv": null, "stagesJSON": null, "userEdited": false, "startTsAdjusted": null,
            "motionJSON": null, "sleepStateJSON": null, "stagingSparse": null,
        }),
    )
}

pub(super) fn workout(index: i64, sport: &str, end_offset: i64) -> Value {
    record(
        json!({"startTs": ts(index), "sport": sport}),
        json!({
            "endTs": ts(index) + end_offset, "source": "noop", "durationS": null,
            "energyKcal": null, "avgHr": null, "maxHr": null, "strain": null, "distanceM": null,
            "zonesJSON": null, "notes": null, "routePolyline": null, "steps": null,
        }),
    )
}

pub(super) fn journal(index: i64, question: &str, answered_yes: bool) -> Value {
    record(
        json!({"day": day(index), "question": question}),
        json!({"answeredYes": answered_yes, "notes": null, "numericValue": null}),
    )
}

pub(super) async fn daily_recoveries(db: &SqlitePool) -> Vec<Value> {
    rows(
        db,
        "SELECT json_object('day', day, 'recovery', recovery) FROM daily_metric \
         WHERE source_id = '3a3486dd-5030-4e17-a00d-a781399890f9' \
         AND device_id = 'strap-local-id' ORDER BY day",
    )
    .await
}

pub(super) fn recovery_rows(expected: &[(i64, f64)]) -> Vec<Value> {
    expected
        .iter()
        .map(|(index, recovery)| json!({"day": day(*index), "recovery": recovery}))
        .collect()
}

/// Row counts of every replace-window table, to prove a rejected part changed nothing.
pub(super) async fn replace_state(db: &SqlitePool) -> Vec<i64> {
    let mut counts = Vec::new();
    for table in [
        "daily_metric",
        "sleep_session",
        "workout",
        "journal",
        "batch_ledger",
        "replacement",
        "replacement_scope",
        "replacement_part",
    ] {
        counts.push(count(db, &format!("SELECT COUNT(*) FROM {table}")).await);
    }
    counts.push(
        count(
            db,
            "SELECT COUNT(*) FROM replacement_part WHERE entity IS NOT NULL",
        )
        .await,
    );
    counts.push(
        count(
            db,
            "SELECT COUNT(*) FROM replacement WHERE state = 'applied'",
        )
        .await,
    );
    counts
}
