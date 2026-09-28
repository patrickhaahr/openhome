mod common;

use axum::Router;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use openhome_api::routes::noop_push::{PUSH_PATH, PushState, router as push_router};
use openhome_api::services::noop_push::{self, PushToken, V1_STREAMS};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use sqlx::SqlitePool;
use sqlx::sqlite::SqlitePoolOptions;
use tower::ServiceExt;

const PUSH_TOKEN: &str = "test-push-token";
const API_KEY: &str = "test-api-key";

async fn memory_push_db() -> SqlitePool {
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
async fn app_with_push_db(push_db: SqlitePool) -> Router {
    let (api, _) = common::test_app_with_db().await;
    let push_state = PushState {
        db: push_db,
        token: PushToken::new(PUSH_TOKEN.to_string()).unwrap(),
    };

    api.merge(push_router(push_state))
}

async fn app() -> Router {
    app_with_push_db(memory_push_db().await).await
}

struct Reply {
    status: StatusCode,
    headers: http::HeaderMap,
    raw: Vec<u8>,
    json: Value,
}

async fn get(app: Router, uri: &str, headers: &[(&str, &str)]) -> Reply {
    let mut builder = Request::builder().method(Method::GET).uri(uri);
    for (name, value) in headers {
        builder = builder.header(*name, *value);
    }
    let response = app
        .oneshot(builder.body(Body::empty()).unwrap())
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

fn bearer(token: &str) -> String {
    format!("Bearer {token}")
}

async fn capabilities(app: Router, version: &str, token: &str) -> Reply {
    let auth = bearer(token);
    get(
        app,
        PUSH_PATH,
        &[
            ("accept", "application/json"),
            ("authorization", &auth),
            ("noop-push-accept-version", version),
        ],
    )
    .await
}

fn assert_canonical_uuid(value: &Value) -> String {
    let id = value.as_str().expect("receiverStateId is a string");
    let parsed = uuid::Uuid::parse_str(id).expect("receiverStateId is a UUID");
    assert_eq!(id, parsed.hyphenated().to_string(), "canonical lowercase");
    id.to_string()
}

#[tokio::test]
async fn returns_capabilities_with_all_v1_streams() {
    let reply = capabilities(app().await, "1.0", PUSH_TOKEN).await;

    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(
        reply.headers[header::CONTENT_TYPE].to_str().unwrap(),
        "application/json"
    );
    let receiver_state_id = assert_canonical_uuid(&reply.json["receiverStateId"]);
    assert_eq!(
        reply.json,
        json!({
            "type": "capabilities",
            "protocolVersion": "1.0",
            "receiverStateId": receiver_state_id,
            "streams": [
                "hrSample", "rrInterval", "event", "battery", "spo2Sample", "skinTempSample",
                "respSample", "gravitySample", "dailyMetric", "sleepSession", "workout", "journal"
            ],
        })
    );
    assert!(reply.raw.len() < 16 * 1024);
    assert_eq!(V1_STREAMS.len(), 12);
}

#[tokio::test]
async fn selects_first_supported_version_in_sender_order() {
    for offer in ["2.0, 1.0", "1.1,1.0", " 1.0 ", "9.9, 1.0, 1.1"] {
        let reply = capabilities(app().await, offer, PUSH_TOKEN).await;

        assert_eq!(reply.status, StatusCode::OK, "{offer}");
        assert_eq!(reply.json["protocolVersion"], "1.0", "{offer}");
    }
}

#[tokio::test]
async fn unsupported_or_missing_version_is_406_before_auth() {
    let offers: [Option<&str>; 4] = [None, Some(""), Some("2.0, 1.1"), Some("1")];
    for offer in offers {
        for token in [Some(PUSH_TOKEN), Some("wrong"), None] {
            let auth = token.map(bearer);
            let mut headers = vec![];
            if let Some(offer) = offer {
                headers.push(("noop-push-accept-version", offer));
            }
            if let Some(auth) = auth.as_deref() {
                headers.push(("authorization", auth));
            }

            let reply = get(app().await, PUSH_PATH, &headers).await;

            assert_eq!(reply.status, StatusCode::NOT_ACCEPTABLE, "{offer:?}");
            assert_eq!(
                reply.json,
                json!({"type": "error", "protocolVersion": "1.0", "code": "unsupported_version"})
            );
            assert!(reply.raw.len() < 256);
        }
    }
}

#[tokio::test]
async fn missing_or_invalid_token_is_401_and_never_echoed() {
    let cases: [&[(&str, &str)]; 5] = [
        &[],
        &[("authorization", "Bearer wrong-token")],
        &[("authorization", "Bearer ")],
        &[("authorization", "Basic dGVzdC1wdXNoLXRva2Vu")],
        &[("authorization", "Bearer test-api-key")],
    ];
    for auth in cases {
        let mut headers = vec![("noop-push-accept-version", "1.0")];
        headers.extend_from_slice(auth);

        let reply = get(app().await, PUSH_PATH, &headers).await;

        assert_eq!(reply.status, StatusCode::UNAUTHORIZED, "{auth:?}");
        assert_eq!(reply.headers[header::WWW_AUTHENTICATE], "Bearer");
        assert_eq!(
            reply.json,
            json!({"type": "error", "protocolVersion": "1.0", "code": "unauthorized"})
        );
        let body = String::from_utf8(reply.raw).unwrap();
        assert!(!body.contains("token") && !body.contains("api-key"));
    }
}

#[tokio::test]
async fn push_token_does_not_unlock_the_main_api() {
    let reply = get(
        app().await,
        "/api/health",
        &[("authorization", &bearer(PUSH_TOKEN))],
    )
    .await;

    assert_eq!(reply.status, StatusCode::UNAUTHORIZED);

    let reply = get(
        app().await,
        "/api/health",
        &[("authorization", &bearer(API_KEY))],
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK);
}

#[tokio::test]
async fn unknown_push_paths_are_404() {
    for uri in ["/api/noop/push/extra", "/api/noop/other", "/api/noop"] {
        let reply = get(
            app().await,
            uri,
            &[
                ("noop-push-accept-version", "1.0"),
                ("authorization", &bearer(PUSH_TOKEN)),
            ],
        )
        .await;

        assert_eq!(reply.status, StatusCode::NOT_FOUND, "{uri}");
        assert_eq!(reply.json["code"], "not_found", "{uri}");
    }
}

#[tokio::test]
async fn receiver_state_id_is_stable_and_follows_rotation() {
    let db = memory_push_db().await;
    let first = capabilities(app_with_push_db(db.clone()).await, "1.0", PUSH_TOKEN).await;
    let second = capabilities(app_with_push_db(db.clone()).await, "1.0", PUSH_TOKEN).await;
    assert_eq!(
        first.json["receiverStateId"],
        second.json["receiverStateId"]
    );

    // Re-running initialization (as on restart) must not replace the persisted ID.
    noop_push::initialize(&db).await.unwrap();
    let after_restart = capabilities(app_with_push_db(db.clone()).await, "1.0", PUSH_TOKEN).await;
    assert_eq!(
        first.json["receiverStateId"],
        after_restart.json["receiverStateId"]
    );

    let rotated = "5FC7B9A0-8055-4E49-A308-3A290F98D81A";
    sqlx::query("UPDATE receiver_state SET receiver_state_id = ? WHERE id = 1")
        .bind(rotated)
        .execute(&db)
        .await
        .unwrap();
    let reply = capabilities(app_with_push_db(db).await, "1.0", PUSH_TOKEN).await;
    assert_eq!(
        reply.json["receiverStateId"],
        rotated.to_ascii_lowercase(),
        "rotation is picked up and canonicalized"
    );
}

#[tokio::test]
async fn receiver_state_id_persists_across_reconnects() {
    let path = std::env::temp_dir().join(format!("noop-push-{}.db", uuid::Uuid::new_v4()));
    let url = format!("sqlite:{}", path.display());

    let first = noop_push::receiver_state_id(&noop_push::connect(&url).await.unwrap())
        .await
        .unwrap();
    let second = noop_push::receiver_state_id(&noop_push::connect(&url).await.unwrap())
        .await
        .unwrap();

    assert_eq!(first, second);
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
    }
}

#[tokio::test]
async fn baseline_retry_converges_after_restart_and_database_backup() {
    let path = std::env::temp_dir().join(format!("noop-restart-{}.db", uuid::Uuid::new_v4()));
    let backup = path.with_extension("backup.db");
    let url = format!("sqlite:{}", path.display());
    let append = hr_batch(BATCH_1, &[hr(1, 70)]);
    let replacement_id = id(100);
    let part_1 = replace_part(
        "dailyMetric",
        &id(1),
        day_window(&replacement_id, 1, 4, 1, 2),
        &[daily(1, 50.0)],
    );
    let part_2 = replace_part(
        "dailyMetric",
        &id(2),
        day_window(&replacement_id, 1, 4, 2, 2),
        &[daily(2, 60.0)],
    );

    let db = noop_push::connect(&url).await.unwrap();
    let state_id = noop_push::receiver_state_id(&db).await.unwrap();
    let append_ack = post(&db, &append, true).await;
    let staged_ack = post(&db, &part_1, true).await;
    assert_eq!(append_ack.status, StatusCode::OK);
    assert_eq!(staged_ack.status, StatusCode::OK);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM hr_sample").await, 1);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM daily_metric").await, 0);
    db.close().await;

    // Startup runs migrations again against the same file. The sender lost both acks.
    let db = noop_push::connect(&url).await.unwrap();
    assert_eq!(noop_push::receiver_state_id(&db).await.unwrap(), state_id);
    assert_eq!(post(&db, &append, false).await.raw, append_ack.raw);
    assert_eq!(post(&db, &part_1, false).await.raw, staged_ack.raw);
    assert_error(
        &post(&db, &hr_batch(BATCH_1, &[hr(1, 71)]), false).await,
        StatusCode::CONFLICT,
        "batch_conflict",
    );
    assert_error(
        &post(
            &db,
            &replace_part(
                "dailyMetric",
                &id(1),
                day_window(&replacement_id, 1, 4, 1, 2),
                &[daily(1, 51.0)],
            ),
            false,
        )
        .await,
        StatusCode::CONFLICT,
        "batch_conflict",
    );
    assert_eq!(count(&db, "SELECT COUNT(*) FROM hr_sample").await, 1);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM batch_ledger").await, 2);
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM replacement_part WHERE entity IS NOT NULL"
        )
        .await,
        1,
    );
    assert_eq!(post(&db, &part_2, true).await.status, StatusCode::OK);
    assert_eq!(
        daily_recoveries(&db).await,
        recovery_rows(&[(1, 50.0), (2, 60.0)]),
    );
    db.close().await;

    // A closed SQLite file is a consistent backup containing both data and protocol state.
    std::fs::copy(&path, &backup).unwrap();
    let backup_db = noop_push::connect(&format!("sqlite:{}", backup.display()))
        .await
        .unwrap();
    assert_eq!(
        noop_push::receiver_state_id(&backup_db).await.unwrap(),
        state_id
    );
    assert_eq!(count(&backup_db, "SELECT COUNT(*) FROM hr_sample").await, 1);
    assert_eq!(
        count(&backup_db, "SELECT COUNT(*) FROM batch_ledger").await,
        3
    );
    assert_eq!(
        daily_recoveries(&backup_db).await,
        recovery_rows(&[(1, 50.0), (2, 60.0)]),
    );
    assert_eq!(
        post(&backup_db, &part_2, false).await.status,
        StatusCode::OK
    );
    backup_db.close().await;

    for file in [&path, &backup] {
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", file.display()));
        }
    }
}

// ---------------------------------------------------------------------------------------------
// POST: append batch ingest
// ---------------------------------------------------------------------------------------------

const SOURCE_A: &str = "3a3486dd-5030-4e17-a00d-a781399890f9";
const SOURCE_B: &str = "7c9e6679-7425-40de-944b-e07fc1f90ae7";
const DEVICE: &str = "strap-local-id";
const BATCH_1: &str = "e835f32f-60e7-4c93-90a0-51eb6830119a";
const BATCH_2: &str = "0f8fad5b-d9cb-469f-a165-70867728950e";
const START_SHA: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const END_SHA: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const MAX_DECODED: usize = 4 * 1024 * 1024;

fn push_app(db: SqlitePool) -> Router {
    push_router(PushState {
        db,
        token: PushToken::new(PUSH_TOKEN.to_string()).unwrap(),
    })
}

fn cursor(row_id: i64, key_sha256: &str) -> Value {
    json!({"rowId": row_id, "keySha256": key_sha256})
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "JSON builders take `json!` literals by value, as `json!` itself does"
)]
fn record(key: Value, data: Value) -> Value {
    json!({"type": "record", "key": key, "data": data})
}

fn hr(ts: i64, bpm: i64) -> Value {
    record(json!({"ts": ts}), json!({"bpm": bpm}))
}

/// A valid append header for `count` records at insertion positions 101..=100+count.
fn header(stream: &str, batch_id: &str, count: usize) -> Value {
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

fn ndjson(header: &Value, records: &[Value]) -> Vec<u8> {
    let mut entity = String::new();
    for line in std::iter::once(header).chain(records) {
        entity.push_str(&line.to_string());
        entity.push('\n');
    }
    entity.into_bytes()
}

fn hr_batch(batch_id: &str, records: &[Value]) -> Vec<u8> {
    ndjson(&header("hrSample", batch_id, records.len()), records)
}

fn gzip(bytes: &[u8]) -> Vec<u8> {
    use std::io::Write;
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(bytes).unwrap();
    encoder.finish().unwrap()
}

async fn send(app: Router, headers: &[(&str, &str)], body: Vec<u8>) -> Reply {
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
async fn post(db: &SqlitePool, entity: &[u8], gzipped: bool) -> Reply {
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
fn expected_ack(stream: &str, batch_id: &str, end_cursor: Value, rows: usize) -> Value {
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
fn assert_error(reply: &Reply, status: StatusCode, code: &str) {
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

async fn count(db: &SqlitePool, sql: &str) -> i64 {
    sqlx::query_scalar(sql).fetch_one(db).await.unwrap()
}

async fn rows(db: &SqlitePool, sql: &str) -> Vec<Value> {
    let rows: Vec<String> = sqlx::query_scalar(sql).fetch_all(db).await.unwrap();
    rows.iter()
        .map(|row| serde_json::from_str(row).unwrap())
        .collect()
}

#[tokio::test]
async fn stores_every_append_stream_columnar_and_acks_exactly() {
    let cases = [
        (
            "hrSample",
            record(json!({"ts": 1723939201}), json!({"bpm": 61})),
            "SELECT json_object('source_id', source_id, 'device_id', device_id, 'ts', ts, \
             'bpm', bpm) FROM hr_sample",
            json!({"source_id": SOURCE_A, "device_id": DEVICE, "ts": 1723939201, "bpm": 61}),
        ),
        (
            "rrInterval",
            record(
                json!({"ts": 100, "rrMs": 812, "seq": 1}),
                json!({"ord": 3, "srcChannel": null, "tsSuspect": 0}),
            ),
            "SELECT json_object('source_id', source_id, 'device_id', device_id, 'ts', ts, \
             'rr_ms', rr_ms, 'seq', seq, 'ord', ord, 'src_channel', src_channel, \
             'ts_suspect', ts_suspect) FROM rr_interval",
            json!({"source_id": SOURCE_A, "device_id": DEVICE, "ts": 100, "rr_ms": 812,
                   "seq": 1, "ord": 3, "src_channel": null, "ts_suspect": 0}),
        ),
        (
            "event",
            record(
                json!({"ts": 100, "kind": "BATTERY_LEVEL(3)"}),
                json!({"payloadJSON": "{\"level\":80}"}),
            ),
            "SELECT json_object('source_id', source_id, 'device_id', device_id, 'ts', ts, \
             'kind', kind, 'payload_json', payload_json) FROM event",
            json!({"source_id": SOURCE_A, "device_id": DEVICE, "ts": 100,
                   "kind": "BATTERY_LEVEL(3)", "payload_json": "{\"level\":80}"}),
        ),
        (
            "battery",
            record(
                json!({"ts": 101}),
                json!({"soc": 80.5, "mv": null, "charging": true}),
            ),
            "SELECT json_object('source_id', source_id, 'device_id', device_id, 'ts', ts, \
             'soc', soc, 'mv', mv, 'charging', charging) FROM battery",
            json!({"source_id": SOURCE_A, "device_id": DEVICE, "ts": 101, "soc": 80.5,
                   "mv": null, "charging": 1}),
        ),
        (
            "spo2Sample",
            record(json!({"ts": 102}), json!({"red": 12345, "ir": 23456})),
            "SELECT json_object('source_id', source_id, 'device_id', device_id, 'ts', ts, \
             'red', red, 'ir', ir) FROM spo2_sample",
            json!({"source_id": SOURCE_A, "device_id": DEVICE, "ts": 102, "red": 12345,
                   "ir": 23456}),
        ),
        (
            "skinTempSample",
            record(
                json!({"ts": 103}),
                json!({"raw": 1000, "aux1Raw": null, "aux2Raw": 7}),
            ),
            "SELECT json_object('source_id', source_id, 'device_id', device_id, 'ts', ts, \
             'raw', raw, 'aux1_raw', aux1_raw, 'aux2_raw', aux2_raw) FROM skin_temp_sample",
            json!({"source_id": SOURCE_A, "device_id": DEVICE, "ts": 103, "raw": 1000,
                   "aux1_raw": null, "aux2_raw": 7.0}),
        ),
        (
            "respSample",
            record(json!({"ts": 104}), json!({"raw": -5})),
            "SELECT json_object('source_id', source_id, 'device_id', device_id, 'ts', ts, \
             'raw', raw) FROM resp_sample",
            json!({"source_id": SOURCE_A, "device_id": DEVICE, "ts": 104, "raw": -5}),
        ),
        (
            "gravitySample",
            record(
                json!({"ts": 105}),
                json!({"x": 0.01, "y": -0.98, "z": 1, "dynAccel": null}),
            ),
            "SELECT json_object('source_id', source_id, 'device_id', device_id, 'ts', ts, \
             'x', x, 'y', y, 'z', z, 'dyn_accel', dyn_accel) FROM gravity_sample",
            json!({"source_id": SOURCE_A, "device_id": DEVICE, "ts": 105, "x": 0.01,
                   "y": -0.98, "z": 1.0, "dyn_accel": null}),
        ),
    ];

    for (stream, record, select, expected_row) in cases {
        let db = memory_push_db().await;
        let entity = ndjson(&header(stream, BATCH_1, 1), &[record]);

        let reply = post(&db, &entity, true).await;

        assert_eq!(reply.status, StatusCode::OK, "{stream}: {:?}", reply.json);
        assert_eq!(
            reply.headers[header::CONTENT_TYPE].to_str().unwrap(),
            "application/json"
        );
        assert_eq!(
            reply.json,
            expected_ack(stream, BATCH_1, cursor(101, END_SHA), 1),
            "{stream}"
        );
        assert_eq!(rows(&db, select).await, vec![expected_row], "{stream}");
    }
}

#[tokio::test]
async fn multi_record_batch_acks_record_count_and_echoes_key_sha_verbatim() {
    let db = memory_push_db().await;
    // keySha256 is opaque: it need not match the records and is echoed as sent.
    let sha = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    let mut batch_header = header("hrSample", BATCH_1, 3);
    batch_header["startCursor"] = Value::Null;
    batch_header["endCursor"] = cursor(48119, sha);
    let entity = ndjson(&batch_header, &[hr(3, 63), hr(1, 61), hr(2, 62)]);

    let reply = post(&db, &entity, false).await;

    assert_eq!(reply.status, StatusCode::OK, "{:?}", reply.json);
    assert_eq!(
        reply.json,
        expected_ack("hrSample", BATCH_1, cursor(48119, sha), 3)
    );
    assert_eq!(count(&db, "SELECT COUNT(*) FROM hr_sample").await, 3);
}

#[tokio::test]
async fn byte_identical_retry_replays_the_ack_across_encodings_without_reapplying() {
    let db = memory_push_db().await;
    let entity = hr_batch(BATCH_1, &[hr(1, 61), hr(2, 62)]);

    let first = post(&db, &entity, true).await;
    assert_eq!(first.status, StatusCode::OK);

    // Tamper with a stored row: a replay must not re-apply the batch.
    sqlx::query("UPDATE hr_sample SET bpm = 0 WHERE ts = 1")
        .execute(&db)
        .await
        .unwrap();

    for gzipped in [false, true] {
        let retry = post(&db, &entity, gzipped).await;
        assert_eq!(retry.status, StatusCode::OK, "gzip={gzipped}");
        assert_eq!(retry.raw, first.raw, "gzip={gzipped}");
    }
    assert_eq!(
        count(&db, "SELECT bpm FROM hr_sample WHERE ts = 1").await,
        0,
        "replay did not re-apply rows"
    );
    assert_eq!(count(&db, "SELECT COUNT(*) FROM hr_sample").await, 2);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM batch_ledger").await, 1);
}

#[tokio::test]
async fn identity_first_then_gzip_retry_is_the_same_batch() {
    let db = memory_push_db().await;
    let entity = hr_batch(BATCH_1, &[hr(1, 61)]);

    let first = post(&db, &entity, false).await;
    let retry = post(&db, &entity, true).await;

    assert_eq!(first.status, StatusCode::OK);
    assert_eq!(retry.status, StatusCode::OK);
    assert_eq!(retry.raw, first.raw);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM batch_ledger").await, 1);
}

#[tokio::test]
async fn reused_batch_id_with_different_bytes_is_409_without_data_change() {
    let db = memory_push_db().await;
    assert_eq!(
        post(&db, &hr_batch(BATCH_1, &[hr(1, 61)]), true)
            .await
            .status,
        StatusCode::OK
    );

    let conflicting = [
        hr_batch(BATCH_1, &[hr(1, 99)]),
        hr_batch(BATCH_1, &[hr(2, 61)]),
        // Same JSON values, different bytes (member order): still a different entity.
        format!(
            "{}\n{}\n",
            header("hrSample", BATCH_1, 1),
            r#"{"key":{"ts":1},"type":"record","data":{"bpm":61}}"#
        )
        .into_bytes(),
    ];
    for entity in conflicting {
        let reply = post(&db, &entity, true).await;
        assert_error(&reply, StatusCode::CONFLICT, "batch_conflict");
    }

    assert_eq!(
        rows(
            &db,
            "SELECT json_object('ts', ts, 'bpm', bpm) FROM hr_sample"
        )
        .await,
        vec![json!({"ts": 1, "bpm": 61})]
    );
    assert_eq!(count(&db, "SELECT COUNT(*) FROM batch_ledger").await, 1);
}

#[tokio::test]
async fn batch_ids_are_scoped_by_source_and_device() {
    let db = memory_push_db().await;
    let entity = hr_batch(BATCH_1, &[hr(1, 61)]);
    assert_eq!(post(&db, &entity, true).await.status, StatusCode::OK);

    let mut other_source = header("hrSample", BATCH_1, 1);
    other_source["sourceId"] = json!(SOURCE_B);
    let mut other_device = header("hrSample", BATCH_1, 1);
    other_device["deviceId"] = json!("other-strap");
    for batch_header in [other_source, other_device] {
        let reply = post(&db, &ndjson(&batch_header, &[hr(1, 70)]), true).await;
        assert_eq!(reply.status, StatusCode::OK, "{:?}", reply.json);
    }

    assert_eq!(count(&db, "SELECT COUNT(*) FROM batch_ledger").await, 3);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM hr_sample").await, 3);
}

#[tokio::test]
async fn upsert_updates_by_scoped_key_and_sources_never_overwrite_each_other() {
    let db = memory_push_db().await;
    assert_eq!(
        post(&db, &hr_batch(BATCH_1, &[hr(1, 60)]), true)
            .await
            .status,
        StatusCode::OK
    );
    assert_eq!(
        post(&db, &hr_batch(BATCH_2, &[hr(1, 70)]), true)
            .await
            .status,
        StatusCode::OK
    );
    let mut source_b = header("hrSample", BATCH_1, 1);
    source_b["sourceId"] = json!(SOURCE_B);
    assert_eq!(
        post(&db, &ndjson(&source_b, &[hr(1, 80)]), true)
            .await
            .status,
        StatusCode::OK
    );

    assert_eq!(
        rows(
            &db,
            "SELECT json_object('source_id', source_id, 'ts', ts, 'bpm', bpm) \
             FROM hr_sample ORDER BY source_id"
        )
        .await,
        vec![
            json!({"source_id": SOURCE_A, "ts": 1, "bpm": 70}),
            json!({"source_id": SOURCE_B, "ts": 1, "bpm": 80}),
        ]
    );
}

#[tokio::test]
async fn rotating_receiver_state_id_starts_a_new_idempotency_generation() {
    let db = memory_push_db().await;
    let entity = hr_batch(BATCH_1, &[hr(1, 61)]);
    assert_eq!(post(&db, &entity, true).await.status, StatusCode::OK);
    sqlx::query("UPDATE hr_sample SET bpm = 0")
        .execute(&db)
        .await
        .unwrap();

    sqlx::query("UPDATE receiver_state SET receiver_state_id = ? WHERE id = 1")
        .bind(uuid::Uuid::new_v4().hyphenated().to_string())
        .execute(&db)
        .await
        .unwrap();
    let retry = post(&db, &entity, true).await;

    assert_eq!(retry.status, StatusCode::OK);
    assert_eq!(
        count(&db, "SELECT bpm FROM hr_sample WHERE ts = 1").await,
        61,
        "baseline re-applied after rotation"
    );
}

#[tokio::test]
async fn malformed_ndjson_is_400() {
    let valid = String::from_utf8(hr_batch(BATCH_1, &[hr(1, 61)])).unwrap();
    let header_line = valid.lines().next().unwrap().to_string();
    let record_line = hr(1, 61).to_string();
    let two_records = String::from_utf8(hr_batch(BATCH_1, &[hr(1, 61), hr(2, 62)])).unwrap();
    let two_header = two_records.lines().next().unwrap().to_string();

    let cases: Vec<(&str, Vec<u8>, &str)> = vec![
        ("empty body", vec![], "malformed_ndjson"),
        (
            "missing final LF",
            valid.trim_end().as_bytes().to_vec(),
            "malformed_ndjson",
        ),
        (
            "blank line",
            format!("{header_line}\n\n{record_line}\n").into_bytes(),
            "malformed_ndjson",
        ),
        (
            "trailing blank line",
            format!("{valid}\n").into_bytes(),
            "malformed_ndjson",
        ),
        (
            "CRLF",
            valid.replace('\n', "\r\n").into_bytes(),
            "malformed_ndjson",
        ),
        (
            "BOM",
            [b"\xEF\xBB\xBF".as_slice(), valid.as_bytes()].concat(),
            "malformed_ndjson",
        ),
        (
            "JSON array",
            format!("[{header_line},{record_line}]\n").into_bytes(),
            "malformed_ndjson",
        ),
        (
            "broken record JSON",
            format!("{header_line}\n{{\"type\":\"record\",\"key\":{{\"ts\":1}}\n").into_bytes(),
            "malformed_ndjson",
        ),
        (
            "broken header JSON",
            format!("{{\"type\":\"batch\",}}\n{record_line}\n").into_bytes(),
            "malformed_ndjson",
        ),
        (
            "fewer lines than recordCount",
            format!("{two_header}\n{record_line}\n").into_bytes(),
            "record_count_mismatch",
        ),
        (
            "more lines than recordCount",
            format!("{header_line}\n{record_line}\n{}\n", hr(2, 62)).into_bytes(),
            "record_count_mismatch",
        ),
    ];

    for (name, entity, code) in cases {
        let db = memory_push_db().await;
        let reply = post(&db, &entity, false).await;
        assert_error(&reply, StatusCode::BAD_REQUEST, code);
        assert_eq!(
            count(&db, "SELECT COUNT(*) FROM hr_sample").await,
            0,
            "{name}"
        );
    }
}

#[tokio::test]
async fn corrupt_gzip_is_400() {
    let db = memory_push_db().await;
    let auth = bearer(PUSH_TOKEN);
    let reply = send(
        push_app(db),
        &[
            ("authorization", &auth),
            ("content-type", "application/x-ndjson"),
            ("content-encoding", "gzip"),
        ],
        b"definitely not gzip".to_vec(),
    )
    .await;
    assert_error(&reply, StatusCode::BAD_REQUEST, "invalid_content_encoding");
}

#[tokio::test]
async fn unsupported_or_invalid_batches_are_422() {
    let with = |edit: &dyn Fn(&mut Value)| {
        let mut batch_header = header("hrSample", BATCH_1, 1);
        edit(&mut batch_header);
        ndjson(&batch_header, &[hr(1, 61)])
    };
    let hr_record = |key: Value, data: Value| hr_batch(BATCH_1, &[record(key, data)]);

    let cases: Vec<(&str, Vec<u8>, &str)> = vec![
        (
            "future protocol",
            with(&|h| h["protocolVersion"] = json!("2.0")),
            "unsupported_protocol_version",
        ),
        (
            "missing protocol",
            with(&|h| {
                h.as_object_mut().unwrap().remove("protocolVersion");
            }),
            "unsupported_protocol_version",
        ),
        (
            "unknown stream",
            with(&|h| h["stream"] = json!("ppgHrSample")),
            "unsupported_stream",
        ),
        (
            "mutable stream with append delivery",
            with(&|h| h["stream"] = json!("dailyMetric")),
            "invalid_header",
        ),
        (
            "not a batch header",
            with(&|h| h["type"] = json!("record")),
            "invalid_header",
        ),
        (
            "replace_window delivery",
            with(&|h| h["delivery"] = json!("replace_window")),
            "invalid_header",
        ),
        (
            "window on append",
            with(&|h| h["window"] = Value::Null),
            "invalid_header",
        ),
        (
            "uppercase batchId",
            with(&|h| h["batchId"] = json!(BATCH_1.to_uppercase())),
            "invalid_header",
        ),
        (
            "non-UUID sourceId",
            with(&|h| h["sourceId"] = json!("install-1")),
            "invalid_header",
        ),
        (
            "empty deviceId",
            with(&|h| h["deviceId"] = json!("")),
            "invalid_header",
        ),
        (
            "missing startCursor member",
            with(&|h| {
                h.as_object_mut().unwrap().remove("startCursor");
            }),
            "invalid_header",
        ),
        (
            "fractional recordCount",
            with(&|h| h["recordCount"] = json!(1.5)),
            "invalid_header",
        ),
        (
            "recordCount over 5000",
            with(&|h| h["recordCount"] = json!(5001)),
            "invalid_header",
        ),
        (
            "float for integer",
            hr_record(json!({"ts": 1}), json!({"bpm": 61.0})),
            "invalid_record",
        ),
        (
            "string for integer key",
            hr_record(json!({"ts": "1"}), json!({"bpm": 61})),
            "invalid_record",
        ),
        (
            "null required member",
            hr_record(json!({"ts": 1}), json!({"bpm": null})),
            "invalid_record",
        ),
        (
            "missing data member",
            hr_record(json!({"ts": 1}), json!({})),
            "invalid_record",
        ),
        (
            "missing key column",
            hr_record(json!({}), json!({"bpm": 61})),
            "invalid_record",
        ),
        (
            "extra key column",
            hr_record(json!({"ts": 1, "deviceId": DEVICE}), json!({"bpm": 61})),
            "invalid_record",
        ),
        (
            "wrong record type",
            hr_batch(
                BATCH_1,
                &[json!({"type": "row", "key": {"ts": 1}, "data": {"bpm": 61}})],
            ),
            "invalid_record",
        ),
        (
            "duplicate member in key",
            format!(
                "{}\n{}\n",
                header("hrSample", BATCH_1, 1),
                r#"{"type":"record","key":{"ts":1,"ts":2},"data":{"bpm":61}}"#
            )
            .into_bytes(),
            "invalid_record",
        ),
        (
            "integer beyond i64",
            hr_record(json!({"ts": 1}), json!({"bpm": u64::MAX})),
            "invalid_record",
        ),
        (
            "boolean as integer",
            ndjson(
                &header("battery", BATCH_1, 1),
                &[record(
                    json!({"ts": 1}),
                    json!({"soc": null, "mv": null, "charging": 1}),
                )],
            ),
            "invalid_record",
        ),
        (
            "missing nullable member",
            ndjson(
                &header("battery", BATCH_1, 1),
                &[record(json!({"ts": 1}), json!({"soc": 80.0, "mv": 3700}))],
            ),
            "invalid_record",
        ),
        (
            "nested object instead of JSON text",
            ndjson(
                &header("event", BATCH_1, 1),
                &[record(
                    json!({"ts": 1, "kind": "X"}),
                    json!({"payloadJSON": {"level": 80}}),
                )],
            ),
            "invalid_record",
        ),
        (
            "duplicate key in batch",
            hr_batch(BATCH_1, &[hr(1, 61), hr(1, 62)]),
            "duplicate_key",
        ),
    ];

    for (name, entity, code) in cases {
        let db = memory_push_db().await;
        let reply = post(&db, &entity, true).await;
        assert_eq!(reply.json["code"], code, "{name}");
        assert_error(&reply, StatusCode::UNPROCESSABLE_ENTITY, code);
        assert_eq!(
            count(&db, "SELECT COUNT(*) FROM hr_sample").await,
            0,
            "{name}"
        );
        assert_eq!(
            count(&db, "SELECT COUNT(*) FROM batch_ledger").await,
            0,
            "{name}"
        );
    }
}

#[tokio::test]
async fn unknown_members_are_ignored() {
    let db = memory_push_db().await;
    let mut batch_header = header("hrSample", BATCH_1, 1);
    batch_header["futureHeaderMember"] = json!({"nested": true});
    let entity = ndjson(
        &batch_header,
        &[json!({"type": "record", "key": {"ts": 1}, "data": {"bpm": 61, "future": 1}, "x": 2})],
    );

    let reply = post(&db, &entity, true).await;

    assert_eq!(reply.status, StatusCode::OK, "{:?}", reply.json);
    assert_eq!(
        reply.json,
        expected_ack("hrSample", BATCH_1, cursor(101, END_SHA), 1)
    );
}

#[tokio::test]
async fn empty_append_batch_is_rejected() {
    let db = memory_push_db().await;
    let mut batch_header = header("hrSample", BATCH_1, 0);
    batch_header["endCursor"] = cursor(101, END_SHA);

    let reply = post(&db, &ndjson(&batch_header, &[]), true).await;

    assert_error(&reply, StatusCode::UNPROCESSABLE_ENTITY, "empty_batch");
}

#[tokio::test]
async fn cursors_must_satisfy_the_contract_inequality() {
    // (startCursor, endCursor, records, accepted?)
    let two = [hr(1, 61), hr(2, 62)];
    let cases: Vec<(Value, Value, &[Value], bool)> = vec![
        // Tightest valid range: positions 101, 102.
        (cursor(100, START_SHA), cursor(102, END_SHA), &two, true),
        (Value::Null, cursor(2, END_SHA), &two, true),
        // Too few insertion positions for the records.
        (cursor(100, START_SHA), cursor(101, END_SHA), &two, false),
        (Value::Null, cursor(1, END_SHA), &two, false),
        // endCursor not after startCursor.
        (
            cursor(100, START_SHA),
            cursor(100, END_SHA),
            &two[..1],
            false,
        ),
        (
            cursor(100, START_SHA),
            cursor(50, END_SHA),
            &two[..1],
            false,
        ),
        // Append batches always carry an endCursor.
        (cursor(100, START_SHA), Value::Null, &two[..1], false),
        // Row IDs are positive integers.
        (cursor(0, START_SHA), cursor(5, END_SHA), &two[..1], false),
        (cursor(-5, START_SHA), cursor(5, END_SHA), &two[..1], false),
        // keySha256 must be lowercase SHA-256 hex.
        (
            cursor(100, START_SHA),
            cursor(101, &END_SHA.to_uppercase()),
            &two[..1],
            false,
        ),
        (cursor(100, "abc"), cursor(101, END_SHA), &two[..1], false),
        (
            cursor(100, START_SHA),
            json!({"rowId": 101}),
            &two[..1],
            false,
        ),
        (
            cursor(100, START_SHA),
            json!({"rowId": 101.5, "keySha256": END_SHA}),
            &two[..1],
            false,
        ),
    ];

    for (start, end, records, accepted) in cases {
        let db = memory_push_db().await;
        let mut batch_header = header("hrSample", BATCH_1, records.len());
        batch_header["startCursor"] = start.clone();
        batch_header["endCursor"] = end.clone();

        let reply = post(&db, &ndjson(&batch_header, records), true).await;

        if accepted {
            assert_eq!(reply.status, StatusCode::OK, "{start} {end}");
            assert_eq!(reply.json["endCursor"], end);
        } else {
            assert_eq!(
                reply.status,
                StatusCode::UNPROCESSABLE_ENTITY,
                "{start} {end}"
            );
            assert!(
                ["invalid_cursor", "invalid_header"]
                    .contains(&reply.json["code"].as_str().unwrap()),
                "{start} {end}: {:?}",
                reply.json
            );
        }
    }
}

/// An event batch padded to exactly `size` decoded bytes.
fn event_batch_of_size(size: usize) -> Vec<u8> {
    let entity = |payload: &str| {
        ndjson(
            &header("event", BATCH_1, 1),
            &[record(
                json!({"ts": 1, "kind": "PAD"}),
                json!({"payloadJSON": payload}),
            )],
        )
    };
    let base = entity("").len();
    entity(&"a".repeat(size - base))
}

#[tokio::test]
async fn decoded_body_at_the_4_mib_bound_is_accepted() {
    let db = memory_push_db().await;
    let entity = event_batch_of_size(MAX_DECODED);
    assert_eq!(entity.len(), MAX_DECODED);

    for gzipped in [false, true] {
        let reply = post(&db, &entity, gzipped).await;
        assert_eq!(reply.status, StatusCode::OK, "gzip={gzipped}");
    }
}

#[tokio::test]
async fn decoded_body_over_4_mib_is_413() {
    let db = memory_push_db().await;
    let entity = event_batch_of_size(MAX_DECODED + 1);

    for gzipped in [false, true] {
        let reply = post(&db, &entity, gzipped).await;
        assert_error(&reply, StatusCode::PAYLOAD_TOO_LARGE, "payload_too_large");
    }

    // A tiny gzip entity that expands past the bound is refused while decoding.
    let bomb = vec![b' '; 64 * 1024 * 1024];
    let auth = bearer(PUSH_TOKEN);
    let reply = send(
        push_app(db.clone()),
        &[
            ("authorization", &auth),
            ("content-type", "application/x-ndjson"),
            ("content-encoding", "gzip"),
        ],
        gzip(&bomb),
    )
    .await;
    assert_error(&reply, StatusCode::PAYLOAD_TOO_LARGE, "payload_too_large");
    assert_eq!(count(&db, "SELECT COUNT(*) FROM event").await, 0);
}

#[tokio::test]
async fn batch_post_requires_token_media_type_and_known_coding() {
    let db = memory_push_db().await;
    let entity = hr_batch(BATCH_1, &[hr(1, 61)]);
    let auth = bearer(PUSH_TOKEN);

    for auth_header in [None, Some(bearer("wrong")), Some(bearer(API_KEY))] {
        let mut headers = vec![("content-type", "application/x-ndjson")];
        if let Some(auth_header) = auth_header.as_deref() {
            headers.push(("authorization", auth_header));
        }
        let reply = send(push_app(db.clone()), &headers, entity.clone()).await;
        assert_error(&reply, StatusCode::UNAUTHORIZED, "unauthorized");
    }

    for content_type in [
        None,
        Some("application/json"),
        Some("application/x-ndjson; charset=latin1"),
    ] {
        let mut headers = vec![("authorization", auth.as_str())];
        if let Some(content_type) = content_type {
            headers.push(("content-type", content_type));
        }
        let reply = send(push_app(db.clone()), &headers, entity.clone()).await;
        assert_error(
            &reply,
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "unsupported_media_type",
        );
    }

    for coding in ["br", "deflate", "gzip, gzip"] {
        let reply = send(
            push_app(db.clone()),
            &[
                ("authorization", &auth),
                ("content-type", "application/x-ndjson"),
                ("content-encoding", coding),
            ],
            entity.clone(),
        )
        .await;
        assert_error(
            &reply,
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "unsupported_content_encoding",
        );
    }

    let reply = send(
        push_app(db.clone()),
        &[
            ("authorization", &auth),
            ("content-type", "application/x-ndjson"),
            ("content-encoding", "identity"),
        ],
        entity,
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM hr_sample").await, 1);
}

#[tokio::test]
async fn full_5000_record_batch_is_accepted() {
    let db = memory_push_db().await;
    let records: Vec<Value> = (0..5000).map(|ts| hr(ts, 60)).collect();

    let reply = post(&db, &hr_batch(BATCH_1, &records), true).await;

    assert_eq!(reply.status, StatusCode::OK, "{:?}", reply.json);
    assert_eq!(reply.json["acceptedRows"], 5000);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM hr_sample").await, 5000);
}

#[tokio::test]
async fn skin_temp_aux_readings_accept_any_finite_number() {
    // The protocol does not type aux1Raw/aux2Raw as integers, and the sender emits a REAL
    // storage value as e.g. `12.0`, so fractional and `.0` values must not stall the stream.
    let db = memory_push_db().await;
    let entity = ndjson(
        &header("skinTempSample", BATCH_1, 3),
        &[
            record(
                json!({"ts": 1}),
                json!({"raw": 1000, "aux1Raw": 12.0, "aux2Raw": 12.5}),
            ),
            record(
                json!({"ts": 2}),
                json!({"raw": 1001, "aux1Raw": 3, "aux2Raw": -0.25}),
            ),
            record(
                json!({"ts": 3}),
                json!({"raw": 1002, "aux1Raw": null, "aux2Raw": 1e2}),
            ),
        ],
    );

    let reply = post(&db, &entity, true).await;

    assert_eq!(reply.status, StatusCode::OK, "{:?}", reply.json);
    assert_eq!(
        rows(
            &db,
            "SELECT json_object('ts', ts, 'aux1_raw', aux1_raw, 'aux2_raw', aux2_raw) \
             FROM skin_temp_sample ORDER BY ts"
        )
        .await,
        vec![
            json!({"ts": 1, "aux1_raw": 12.0, "aux2_raw": 12.5}),
            json!({"ts": 2, "aux1_raw": 3.0, "aux2_raw": -0.25}),
            json!({"ts": 3, "aux1_raw": null, "aux2_raw": 100.0}),
        ]
    );

    // `raw` stays an integer.
    let db = memory_push_db().await;
    let entity = ndjson(
        &header("skinTempSample", BATCH_1, 1),
        &[record(
            json!({"ts": 1}),
            json!({"raw": 1000.0, "aux1Raw": null, "aux2Raw": null}),
        )],
    );
    assert_error(
        &post(&db, &entity, true).await,
        StatusCode::UNPROCESSABLE_ENTITY,
        "invalid_record",
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_identical_posts_apply_once_and_return_the_same_ack() {
    let path = std::env::temp_dir().join(format!("noop-push-{}.db", uuid::Uuid::new_v4()));
    let db = noop_push::connect(&format!("sqlite:{}", path.display()))
        .await
        .unwrap();
    let records: Vec<Value> = (0..2000).map(|ts| hr(ts, 60)).collect();
    let entity = hr_batch(BATCH_1, &records);

    let (first, second) = tokio::join!(
        tokio::spawn({
            let (db, entity) = (db.clone(), entity.clone());
            async move { post(&db, &entity, true).await }
        }),
        tokio::spawn({
            let (db, entity) = (db.clone(), entity.clone());
            async move { post(&db, &entity, false).await }
        }),
    );
    let (first, second) = (first.unwrap(), second.unwrap());

    assert_eq!(first.status, StatusCode::OK, "{:?}", first.json);
    assert_eq!(second.status, StatusCode::OK, "{:?}", second.json);
    assert_eq!(first.raw, second.raw);
    assert_eq!(
        first.json,
        expected_ack("hrSample", BATCH_1, cursor(2100, END_SHA), 2000)
    );
    assert_eq!(count(&db, "SELECT COUNT(*) FROM hr_sample").await, 2000);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM batch_ledger").await, 1);

    db.close().await;
    for suffix in ["", "-wal", "-shm", "-journal"] {
        let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
    }
}

// ---------------------------------------------------------------------------------------------
// POST: replace-window delivery for the mutable streams
// ---------------------------------------------------------------------------------------------

/// A distinct canonical UUID per `n` (batch and replacement IDs).
fn id(n: u64) -> String {
    format!("00000000-0000-4000-8000-{n:012x}")
}

fn day(index: i64) -> String {
    format!("2026-08-{index:02}")
}

/// Local-midnight-like Unix seconds for day `index` (UTC here; the receiver never interprets it).
const fn ts(index: i64) -> i64 {
    1_754_006_400 + index * 86_400
}

fn day_window(replacement: &str, start: i64, end: i64, part: u32, parts: u32) -> Value {
    json!({
        "replacementId": replacement,
        "selector": "day",
        "startInclusive": day(start),
        "endExclusive": day(end),
        "part": part,
        "parts": parts,
    })
}

fn ts_window(replacement: &str, start: i64, end: i64, part: u32, parts: u32) -> Value {
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
fn replace_header(stream: &str, batch_id: &str, count: usize, window: Value) -> Value {
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

fn replace_part(stream: &str, batch_id: &str, window: Value, records: &[Value]) -> Vec<u8> {
    ndjson(
        &replace_header(stream, batch_id, records.len(), window),
        records,
    )
}

fn replace_ack(stream: &str, batch_id: &str, rows: usize) -> Value {
    expected_ack(stream, batch_id, Value::Null, rows)
}

#[track_caller]
fn assert_ok(reply: &Reply, expected: &Value) {
    assert_eq!(
        reply.status,
        StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&reply.raw)
    );
    assert_eq!(&reply.json, expected);
}

fn daily(index: i64, recovery: f64) -> Value {
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

fn sleep(index: i64, end_offset: i64) -> Value {
    record(
        json!({"startTs": ts(index)}),
        json!({
            "endTs": ts(index) + end_offset, "efficiency": null, "restingHr": null,
            "avgHrv": null, "stagesJSON": null, "userEdited": false, "startTsAdjusted": null,
            "motionJSON": null, "sleepStateJSON": null, "stagingSparse": null,
        }),
    )
}

fn workout(index: i64, sport: &str, end_offset: i64) -> Value {
    record(
        json!({"startTs": ts(index), "sport": sport}),
        json!({
            "endTs": ts(index) + end_offset, "source": "noop", "durationS": null,
            "energyKcal": null, "avgHr": null, "maxHr": null, "strain": null, "distanceM": null,
            "zonesJSON": null, "notes": null, "routePolyline": null, "steps": null,
        }),
    )
}

fn journal(index: i64, question: &str, answered_yes: bool) -> Value {
    record(
        json!({"day": day(index), "question": question}),
        json!({"answeredYes": answered_yes, "notes": null, "numericValue": null}),
    )
}

async fn daily_recoveries(db: &SqlitePool) -> Vec<Value> {
    rows(
        db,
        "SELECT json_object('day', day, 'recovery', recovery) FROM daily_metric \
         WHERE source_id = '3a3486dd-5030-4e17-a00d-a781399890f9' \
         AND device_id = 'strap-local-id' ORDER BY day",
    )
    .await
}

fn recovery_rows(expected: &[(i64, f64)]) -> Vec<Value> {
    expected
        .iter()
        .map(|(index, recovery)| json!({"day": day(*index), "recovery": recovery}))
        .collect()
}

/// Row counts of every replace-window table, to prove a rejected part changed nothing.
async fn replace_state(db: &SqlitePool) -> Vec<i64> {
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

#[tokio::test]
async fn stores_every_mutable_stream_columnar_and_acks_with_null_end_cursor() {
    let cases = [
        (
            "dailyMetric",
            day_window(&id(1000), 1, 15, 1, 1),
            record(
                json!({"day": "2026-08-05"}),
                json!({
                    "totalSleepMin": 420.5, "efficiency": 0.91, "deepMin": 80.0, "remMin": 95.5,
                    "lightMin": 245.0, "disturbances": 3, "restingHr": 52, "avgHrv": 61.2,
                    "recovery": 77.0, "strain": 12.4, "exerciseCount": 1, "spo2Pct": 96.5,
                    "skinTempDevC": -0.3, "respRateBpm": 14.8, "steps": 9001,
                    "activeKcalEst": 512.5, "spo2Red": 1200.5, "spo2Ir": 1300,
                }),
            ),
            "SELECT json_object('source_id', source_id, 'device_id', device_id, 'day', day, \
             'total_sleep_min', total_sleep_min, 'efficiency', efficiency, 'deep_min', deep_min, \
             'rem_min', rem_min, 'light_min', light_min, 'disturbances', disturbances, \
             'resting_hr', resting_hr, 'avg_hrv', avg_hrv, 'recovery', recovery, \
             'strain', strain, 'exercise_count', exercise_count, 'spo2_pct', spo2_pct, \
             'skin_temp_dev_c', skin_temp_dev_c, 'resp_rate_bpm', resp_rate_bpm, \
             'steps', steps, 'active_kcal_est', active_kcal_est, 'spo2_red', spo2_red, \
             'spo2_ir', spo2_ir) FROM daily_metric",
            json!({
                "source_id": SOURCE_A, "device_id": DEVICE, "day": "2026-08-05",
                "total_sleep_min": 420.5, "efficiency": 0.91, "deep_min": 80.0,
                "rem_min": 95.5, "light_min": 245.0, "disturbances": 3, "resting_hr": 52.0,
                "avg_hrv": 61.2, "recovery": 77.0, "strain": 12.4, "exercise_count": 1,
                "spo2_pct": 96.5, "skin_temp_dev_c": -0.3, "resp_rate_bpm": 14.8,
                "steps": 9001, "active_kcal_est": 512.5, "spo2_red": 1200.5, "spo2_ir": 1300.0,
            }),
        ),
        (
            "sleepSession",
            ts_window(&id(1001), 1, 15, 1, 1),
            record(
                json!({"startTs": ts(5)}),
                json!({
                    "endTs": ts(5) + 28_800, "efficiency": 0.88, "restingHr": 50.5,
                    "avgHrv": 70, "stagesJSON": "[{\"stage\":\"deep\"}]", "userEdited": true,
                    "startTsAdjusted": ts(5) - 60, "motionJSON": "{\"m\":1}",
                    "sleepStateJSON": "[]", "stagingSparse": false,
                }),
            ),
            "SELECT json_object('source_id', source_id, 'device_id', device_id, \
             'start_ts', start_ts, 'end_ts', end_ts, 'efficiency', efficiency, \
             'resting_hr', resting_hr, 'avg_hrv', avg_hrv, 'stages_json', stages_json, \
             'user_edited', user_edited, 'start_ts_adjusted', start_ts_adjusted, \
             'motion_json', motion_json, 'sleep_state_json', sleep_state_json, \
             'staging_sparse', staging_sparse) FROM sleep_session",
            json!({
                "source_id": SOURCE_A, "device_id": DEVICE, "start_ts": ts(5),
                "end_ts": ts(5) + 28_800, "efficiency": 0.88, "resting_hr": 50.5,
                "avg_hrv": 70.0, "stages_json": "[{\"stage\":\"deep\"}]", "user_edited": 1,
                "start_ts_adjusted": ts(5) - 60, "motion_json": "{\"m\":1}",
                "sleep_state_json": "[]", "staging_sparse": 0,
            }),
        ),
        (
            "workout",
            ts_window(&id(1002), 1, 15, 1, 1),
            record(
                json!({"startTs": ts(5), "sport": "running"}),
                json!({
                    "endTs": ts(5) + 3600, "source": "healthConnect", "durationS": 3600,
                    "energyKcal": 640.5, "avgHr": 151, "maxHr": 182.5, "strain": 14.1,
                    "distanceM": 10012.5, "zonesJSON": "{\"z2\":0.4}", "notes": "tempo",
                    "routePolyline": "_p~iF~ps|U", "steps": 9500,
                }),
            ),
            "SELECT json_object('source_id', source_id, 'device_id', device_id, \
             'start_ts', start_ts, 'sport', sport, 'end_ts', end_ts, 'source', source, \
             'duration_s', duration_s, 'energy_kcal', energy_kcal, 'avg_hr', avg_hr, \
             'max_hr', max_hr, 'strain', strain, 'distance_m', distance_m, \
             'zones_json', zones_json, 'notes', notes, 'route_polyline', route_polyline, \
             'steps', steps) FROM workout",
            json!({
                "source_id": SOURCE_A, "device_id": DEVICE, "start_ts": ts(5),
                "sport": "running", "end_ts": ts(5) + 3600, "source": "healthConnect",
                "duration_s": 3600.0, "energy_kcal": 640.5, "avg_hr": 151.0, "max_hr": 182.5,
                "strain": 14.1, "distance_m": 10012.5, "zones_json": "{\"z2\":0.4}",
                "notes": "tempo", "route_polyline": "_p~iF~ps|U", "steps": 9500,
            }),
        ),
        (
            "journal",
            day_window(&id(1003), 1, 15, 1, 1),
            record(
                json!({"day": "2026-08-05", "question": "Caffeine after 2pm?"}),
                json!({"answeredYes": true, "notes": "one espresso", "numericValue": 1.5}),
            ),
            "SELECT json_object('source_id', source_id, 'device_id', device_id, 'day', day, \
             'question', question, 'answered_yes', answered_yes, 'notes', notes, \
             'numeric_value', numeric_value) FROM journal",
            json!({
                "source_id": SOURCE_A, "device_id": DEVICE, "day": "2026-08-05",
                "question": "Caffeine after 2pm?", "answered_yes": 1, "notes": "one espresso",
                "numeric_value": 1.5,
            }),
        ),
    ];

    for (stream, window, row, select, expected) in cases {
        let db = memory_push_db().await;
        let reply = post(&db, &replace_part(stream, BATCH_1, window, &[row]), true).await;

        assert_ok(&reply, &replace_ack(stream, BATCH_1, 1));
        assert_eq!(
            reply.headers[header::CONTENT_TYPE],
            "application/json",
            "{stream}"
        );
        assert_eq!(rows(&db, select).await, vec![expected], "{stream}");
    }
}

#[tokio::test]
async fn each_mutable_stream_replaces_its_window_and_leaves_rows_outside_untouched() {
    type Build = fn(i64, i64) -> Value;
    type Window = fn(&str, i64, i64, u32, u32) -> Value;
    let cases: [(&str, Window, Build, &str); 4] = [
        (
            "dailyMetric",
            day_window,
            |index, variant| daily(index, variant as f64),
            "SELECT json_object('k', day, 'v', recovery) FROM daily_metric ORDER BY day",
        ),
        (
            "sleepSession",
            ts_window,
            |index, variant| sleep(index, variant),
            "SELECT json_object('k', start_ts, 'v', end_ts - start_ts) FROM sleep_session \
             ORDER BY start_ts",
        ),
        (
            "workout",
            ts_window,
            |index, variant| workout(index, "run", variant),
            "SELECT json_object('k', start_ts, 'v', end_ts - start_ts) FROM workout \
             ORDER BY start_ts",
        ),
        (
            "journal",
            day_window,
            |index, variant| journal(index, "coffee", variant == 2),
            "SELECT json_object('k', day, 'v', answered_yes + 1) FROM journal ORDER BY day",
        ),
    ];

    for (stream, window, build, select) in cases {
        let db = memory_push_db().await;
        let key = |index: i64| match stream {
            "dailyMetric" | "journal" => json!(day(index)),
            _ => json!(ts(index)),
        };
        let seed: Vec<Value> = [1, 3, 5, 8].map(|index| build(index, 1)).to_vec();
        let reply = post(
            &db,
            &replace_part(stream, &id(1), window(&id(100), 1, 10, 1, 1), &seed),
            true,
        )
        .await;
        assert_ok(&reply, &replace_ack(stream, &id(1), 4));

        // Window [3, 6): 3 is updated, 4 is new, 5 is absent and deleted; 1 and 8 are outside.
        let replacement = [build(3, 2), build(4, 2)];
        let reply = post(
            &db,
            &replace_part(stream, &id(2), window(&id(101), 3, 6, 1, 1), &replacement),
            false,
        )
        .await;
        assert_ok(&reply, &replace_ack(stream, &id(2), 2));

        let expected: Vec<Value> = [(1, 1), (3, 2), (4, 2), (8, 1)]
            .iter()
            .map(|(index, variant)| {
                let v = if stream == "dailyMetric" {
                    json!(f64::from(*variant))
                } else {
                    json!(variant)
                };
                json!({"k": key(*index), "v": v})
            })
            .collect();
        assert_eq!(rows(&db, select).await, expected, "{stream}");
    }
}

#[tokio::test]
async fn absent_keys_are_deleted_per_full_composite_key() {
    let db = memory_push_db().await;
    let seed = [
        workout(3, "run", 60),
        workout(3, "swim", 60),
        workout(4, "row", 60),
    ];
    let reply = post(
        &db,
        &replace_part("workout", &id(1), ts_window(&id(100), 1, 10, 1, 1), &seed),
        true,
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK);
    let seed = [journal(3, "coffee", true), journal(3, "alcohol", false)];
    let reply = post(
        &db,
        &replace_part("journal", &id(2), day_window(&id(101), 1, 10, 1, 1), &seed),
        true,
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK);

    let reply = post(
        &db,
        &replace_part(
            "workout",
            &id(3),
            ts_window(&id(102), 1, 10, 1, 1),
            &[workout(3, "swim", 90), workout(4, "row", 60)],
        ),
        true,
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK);
    let reply = post(
        &db,
        &replace_part(
            "journal",
            &id(4),
            day_window(&id(103), 1, 10, 1, 1),
            &[journal(3, "alcohol", true)],
        ),
        true,
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK);

    assert_eq!(
        rows(
            &db,
            "SELECT json_object('sport', sport, 'len', end_ts - start_ts) FROM workout \
             ORDER BY start_ts, sport"
        )
        .await,
        vec![
            json!({"sport": "swim", "len": 90}),
            json!({"sport": "row", "len": 60}),
        ]
    );
    assert_eq!(
        rows(
            &db,
            "SELECT json_object('question', question, 'yes', answered_yes) FROM journal"
        )
        .await,
        vec![json!({"question": "alcohol", "yes": 1})]
    );
}

#[tokio::test]
async fn empty_window_deletes_every_row_in_scope_and_window_only() {
    let db = memory_push_db().await;
    let seed: Vec<Value> = (1..=5).map(|index| daily(index, 50.0)).collect();
    for (n, (source, device)) in [
        (SOURCE_A, DEVICE),
        (SOURCE_B, DEVICE),
        (SOURCE_A, "other-strap"),
    ]
    .into_iter()
    .enumerate()
    {
        let mut part_header = replace_header(
            "dailyMetric",
            &id(n as u64 + 1),
            seed.len(),
            day_window(&id(100 + n as u64), 1, 6, 1, 1),
        );
        part_header["sourceId"] = json!(source);
        part_header["deviceId"] = json!(device);
        let reply = post(&db, &ndjson(&part_header, &seed), true).await;
        assert_eq!(reply.status, StatusCode::OK, "{:?}", reply.json);
    }

    let empty = replace_part(
        "dailyMetric",
        BATCH_1,
        day_window(&id(200), 2, 5, 1, 1),
        &[],
    );
    let reply = post(&db, &empty, true).await;

    assert_ok(&reply, &replace_ack("dailyMetric", BATCH_1, 0));
    assert_eq!(
        daily_recoveries(&db).await,
        recovery_rows(&[(1, 50.0), (5, 50.0)])
    );
    assert_eq!(count(&db, "SELECT COUNT(*) FROM daily_metric").await, 12);
}

#[tokio::test]
async fn out_of_order_parts_apply_atomically_once_the_last_part_arrives() {
    let db = memory_push_db().await;
    let seed: Vec<Value> = (1..=9).map(|index| daily(index, 10.0)).collect();
    let reply = post(
        &db,
        &replace_part(
            "dailyMetric",
            &id(1),
            day_window(&id(100), 1, 10, 1, 1),
            &seed,
        ),
        true,
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK);

    // Replacement over [2, 9): day 5 is absent from every part and must be deleted.
    let replacement = id(101);
    let part = |n: u32, records: &[Value]| {
        replace_part(
            "dailyMetric",
            &id(10 + u64::from(n)),
            day_window(&replacement, 2, 9, n, 3),
            records,
        )
    };
    let part_1 = part(1, &[daily(2, 20.0), daily(3, 20.0)]);
    let part_2 = part(2, &[daily(4, 20.0), daily(6, 20.0)]);
    let part_3 = part(3, &[daily(7, 20.0), daily(8, 20.0)]);
    let untouched = recovery_rows(&(1..=9).map(|index| (index, 10.0)).collect::<Vec<_>>());

    let reply = post(&db, &part_3, true).await;
    assert_ok(&reply, &replace_ack("dailyMetric", &id(13), 2));
    assert_eq!(daily_recoveries(&db).await, untouched, "part 3 only staged");
    let reply = post(&db, &part_1, false).await;
    assert_ok(&reply, &replace_ack("dailyMetric", &id(11), 2));
    assert_eq!(
        daily_recoveries(&db).await,
        untouched,
        "parts 1 and 3 staged"
    );
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM replacement_part WHERE entity IS NOT NULL"
        )
        .await,
        2
    );

    let reply = post(&db, &part_2, true).await;

    assert_ok(&reply, &replace_ack("dailyMetric", &id(12), 2));
    assert_eq!(
        daily_recoveries(&db).await,
        recovery_rows(&[
            (1, 10.0),
            (2, 20.0),
            (3, 20.0),
            (4, 20.0),
            (6, 20.0),
            (7, 20.0),
            (8, 20.0),
            (9, 10.0),
        ])
    );
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM replacement_part WHERE entity IS NOT NULL"
        )
        .await,
        0,
        "staged entities are dropped once applied"
    );
    assert_eq!(count(&db, "SELECT COUNT(*) FROM batch_ledger").await, 4);

    // Retrying any part after the apply replays its ack and changes nothing.
    sqlx::query("UPDATE daily_metric SET recovery = 99.0 WHERE day = '2026-08-02'")
        .execute(&db)
        .await
        .unwrap();
    let reply = post(&db, &part_1, true).await;
    assert_ok(&reply, &replace_ack("dailyMetric", &id(11), 2));
    assert_eq!(
        count(
            &db,
            "SELECT CAST(recovery AS INTEGER) FROM daily_metric WHERE day = '2026-08-02'"
        )
        .await,
        99
    );
}

#[tokio::test]
async fn replacement_accepts_more_than_64_parts() {
    let db = memory_push_db().await;
    for part in (1..=65).rev() {
        let batch_id = id(u64::from(part));
        let entity = replace_part(
            "journal",
            &batch_id,
            day_window(&id(100), 1, 2, part, 65),
            &[journal(1, &format!("question-{part}"), true)],
        );
        assert_ok(
            &post(&db, &entity, true).await,
            &replace_ack("journal", &batch_id, 1),
        );
        assert_eq!(
            count(&db, "SELECT COUNT(*) FROM journal").await,
            if part == 1 { 65 } else { 0 },
        );
    }
}

#[tokio::test]
async fn staged_parts_survive_reconnect_and_failed_apply_remains_retryable() {
    let path = std::env::temp_dir().join(format!("noop-replacement-{}.db", uuid::Uuid::new_v4()));
    let url = format!("sqlite:{}", path.display());
    let db = noop_push::connect(&url).await.unwrap();
    let seed = replace_part(
        "dailyMetric",
        &id(1),
        day_window(&id(100), 1, 10, 1, 1),
        &[daily(2, 10.0), daily(5, 10.0)],
    );
    assert_eq!(post(&db, &seed, true).await.status, StatusCode::OK);
    let first = replace_part(
        "dailyMetric",
        &id(2),
        day_window(&id(101), 1, 10, 2, 2),
        &[daily(4, 20.0)],
    );
    let accepted = post(&db, &first, true).await;
    assert_ok(&accepted, &replace_ack("dailyMetric", &id(2), 1));
    db.close().await;

    let db = noop_push::connect(&url).await.unwrap();
    assert_eq!(post(&db, &first, false).await.raw, accepted.raw);
    sqlx::query(
        "CREATE TRIGGER fail_replacement_delete BEFORE DELETE ON daily_metric \
         BEGIN SELECT RAISE(ABORT, 'simulated storage failure'); END",
    )
    .execute(&db)
    .await
    .unwrap();
    let completing = replace_part(
        "dailyMetric",
        &id(3),
        day_window(&id(101), 1, 10, 1, 2),
        &[daily(2, 20.0)],
    );
    let reply = post(&db, &completing, true).await;
    assert_error(
        &reply,
        StatusCode::INTERNAL_SERVER_ERROR,
        "storage_unavailable",
    );
    assert_eq!(
        daily_recoveries(&db).await,
        recovery_rows(&[(2, 10.0), (5, 10.0)])
    );
    assert_eq!(count(&db, "SELECT COUNT(*) FROM batch_ledger").await, 2);
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM replacement_part WHERE entity IS NOT NULL"
        )
        .await,
        1,
    );

    sqlx::query("DROP TRIGGER fail_replacement_delete")
        .execute(&db)
        .await
        .unwrap();
    assert_ok(
        &post(&db, &completing, true).await,
        &replace_ack("dailyMetric", &id(3), 1),
    );
    assert_eq!(
        daily_recoveries(&db).await,
        recovery_rows(&[(2, 20.0), (4, 20.0)])
    );
    db.close().await;
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
    }
}

#[tokio::test]
async fn byte_identical_part_retries_replay_the_ack_across_encodings() {
    let db = memory_push_db().await;
    let single = replace_part(
        "sleepSession",
        BATCH_1,
        ts_window(&id(100), 1, 10, 1, 1),
        &[sleep(2, 100)],
    );
    let first = post(&db, &single, true).await;
    assert_ok(&first, &replace_ack("sleepSession", BATCH_1, 1));
    sqlx::query("UPDATE sleep_session SET end_ts = 0")
        .execute(&db)
        .await
        .unwrap();

    let retry = post(&db, &single, false).await;

    assert_eq!(retry.status, StatusCode::OK);
    assert_eq!(retry.raw, first.raw);
    assert_eq!(
        count(&db, "SELECT end_ts FROM sleep_session").await,
        0,
        "replay does not re-apply"
    );

    // A retried part of a still-incomplete replacement replays too and stays staged.
    let staged = replace_part(
        "sleepSession",
        BATCH_2,
        ts_window(&id(101), 1, 10, 1, 2),
        &[sleep(3, 100)],
    );
    let first = post(&db, &staged, true).await;
    assert_ok(&first, &replace_ack("sleepSession", BATCH_2, 1));
    let before = replace_state(&db).await;
    let retry = post(&db, &staged, false).await;
    assert_eq!(retry.raw, first.raw);
    assert_eq!(replace_state(&db).await, before);
}

#[tokio::test]
async fn conflicting_reuse_is_409_without_data_change() {
    let db = memory_push_db().await;
    let replacement = id(100);
    let window = |part: u32, parts: u32| day_window(&replacement, 1, 10, part, parts);
    let reply = post(
        &db,
        &replace_part(
            "journal",
            &id(1),
            window(1, 2),
            &[journal(2, "coffee", true)],
        ),
        true,
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK);
    let applied = replace_part(
        "dailyMetric",
        &id(2),
        day_window(&id(101), 1, 10, 1, 1),
        &[daily(2, 30.0)],
    );
    assert_eq!(post(&db, &applied, true).await.status, StatusCode::OK);
    let before = replace_state(&db).await;

    let cases: Vec<(&str, Vec<u8>, &str)> = vec![
        (
            "batchId reused with different bytes",
            replace_part(
                "journal",
                &id(1),
                window(1, 2),
                &[journal(3, "coffee", true)],
            ),
            "batch_conflict",
        ),
        (
            "batchId of an applied part reused with different bytes",
            replace_part(
                "dailyMetric",
                &id(2),
                day_window(&id(101), 1, 10, 1, 1),
                &[daily(2, 31.0)],
            ),
            "batch_conflict",
        ),
        (
            "replacementId reused with a different window",
            replace_part(
                "journal",
                &id(3),
                day_window(&replacement, 1, 9, 2, 2),
                &[journal(4, "coffee", true)],
            ),
            "replacement_conflict",
        ),
        (
            "replacementId reused with a different part count",
            replace_part(
                "journal",
                &id(3),
                window(2, 3),
                &[journal(4, "coffee", true)],
            ),
            "replacement_conflict",
        ),
        (
            "replacementId reused for another stream",
            replace_part(
                "dailyMetric",
                &id(3),
                day_window(&replacement, 1, 10, 2, 2),
                &[daily(4, 1.0)],
            ),
            "replacement_conflict",
        ),
        (
            "part number reused with a different batch",
            replace_part(
                "journal",
                &id(3),
                window(1, 2),
                &[journal(3, "coffee", true)],
            ),
            "replacement_conflict",
        ),
        (
            "part number of an applied replacement reused",
            replace_part(
                "dailyMetric",
                &id(3),
                day_window(&id(101), 1, 10, 1, 1),
                &[daily(2, 31.0)],
            ),
            "replacement_conflict",
        ),
    ];

    for (name, entity, code) in cases {
        let reply = post(&db, &entity, true).await;
        assert_eq!(reply.json["code"], code, "{name}");
        assert_error(&reply, StatusCode::CONFLICT, code);
        assert_eq!(replace_state(&db).await, before, "{name}");
    }
    assert_eq!(daily_recoveries(&db).await, recovery_rows(&[(2, 30.0)]));
}

#[tokio::test]
async fn a_new_generation_supersedes_an_incomplete_one_and_late_parts_are_409() {
    let db = memory_push_db().await;
    let old_part_1 = replace_part(
        "dailyMetric",
        &id(1),
        day_window(&id(100), 1, 10, 1, 2),
        &[daily(2, 10.0)],
    );
    let old_part_2 = replace_part(
        "dailyMetric",
        &id(2),
        day_window(&id(100), 1, 10, 2, 2),
        &[daily(3, 10.0)],
    );
    assert_eq!(post(&db, &old_part_1, true).await.status, StatusCode::OK);

    // A different generation (with different bounds) supersedes the incomplete one.
    let new = replace_part(
        "dailyMetric",
        &id(3),
        day_window(&id(101), 2, 8, 1, 1),
        &[daily(4, 20.0)],
    );
    assert_ok(
        &post(&db, &new, true).await,
        &replace_ack("dailyMetric", &id(3), 1),
    );

    for (name, entity) in [("late part", &old_part_2), ("retried part", &old_part_1)] {
        let reply = post(&db, entity, true).await;
        assert_error(&reply, StatusCode::CONFLICT, "replacement_superseded");
        assert_eq!(
            daily_recoveries(&db).await,
            recovery_rows(&[(4, 20.0)]),
            "{name}"
        );
    }
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM replacement_part WHERE entity IS NOT NULL"
        )
        .await,
        0,
        "superseded staging is discarded"
    );

    // Other streams and scopes have independent generations.
    let journal_part = replace_part(
        "journal",
        &id(4),
        day_window(&id(102), 1, 10, 1, 2),
        &[journal(2, "coffee", true)],
    );
    assert_eq!(post(&db, &journal_part, true).await.status, StatusCode::OK);
    let mut other_device = replace_header("journal", &id(5), 1, day_window(&id(103), 1, 10, 1, 2));
    other_device["deviceId"] = json!("other-strap");
    let reply = post(
        &db,
        &ndjson(&other_device, &[journal(2, "tea", true)]),
        true,
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK);
    let journal_part_2 = replace_part(
        "journal",
        &id(6),
        day_window(&id(102), 1, 10, 2, 2),
        &[journal(3, "coffee", true)],
    );
    assert_eq!(
        post(&db, &journal_part_2, true).await.status,
        StatusCode::OK
    );
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM journal WHERE device_id = 'strap-local-id'"
        )
        .await,
        2
    );
}

#[tokio::test]
async fn retrying_an_earlier_applied_replacement_preserves_newer_generations() {
    let db = memory_push_db().await;
    let populated = replace_part(
        "workout",
        &id(1),
        ts_window(&id(100), 3, 4, 1, 1),
        &[workout(3, "run", 60)],
    );
    let emptied = replace_part("workout", &id(2), ts_window(&id(101), 3, 4, 1, 1), &[]);
    let workouts = || count(&db, "SELECT COUNT(*) FROM workout");

    let first_ack = post(&db, &populated, true).await;
    assert_eq!(workouts().await, 1);
    let first_empty_ack = post(&db, &emptied, true).await;
    assert_eq!(workouts().await, 0);

    let again = post(&db, &populated, true).await;
    assert_eq!(again.raw, first_ack.raw);
    assert_eq!(
        workouts().await,
        0,
        "late retry must not restore deleted rows"
    );
    let again = post(&db, &emptied, false).await;
    assert_eq!(again.raw, first_empty_ack.raw);
    assert_eq!(workouts().await, 0, "emptied again");
    assert_eq!(count(&db, "SELECT COUNT(*) FROM batch_ledger").await, 2);

    let pending = replace_part(
        "workout",
        &id(3),
        ts_window(&id(102), 3, 4, 2, 2),
        &[workout(3, "walk", 120)],
    );
    assert_eq!(post(&db, &pending, true).await.status, StatusCode::OK);
    let again = post(&db, &populated, false).await;
    assert_eq!(again.raw, first_ack.raw);
    assert_eq!(
        workouts().await,
        0,
        "retry must not supersede pending parts"
    );
    let completing = replace_part(
        "workout",
        &id(4),
        ts_window(&id(102), 3, 4, 1, 2),
        &[workout(3, "run", 180)],
    );
    assert_ok(
        &post(&db, &completing, true).await,
        &replace_ack("workout", &id(4), 1),
    );
    assert_eq!(workouts().await, 2);
}

#[tokio::test]
async fn invalid_replace_window_parts_are_422() {
    let daily_part = |edit: &dyn Fn(&mut Value)| {
        let mut part_header =
            replace_header("dailyMetric", BATCH_1, 1, day_window(&id(100), 1, 10, 1, 1));
        edit(&mut part_header);
        ndjson(&part_header, &[daily(2, 1.0)])
    };
    let sleep_part = |edit: &dyn Fn(&mut Value)| {
        let mut part_header =
            replace_header("sleepSession", BATCH_1, 1, ts_window(&id(100), 1, 10, 1, 1));
        edit(&mut part_header);
        ndjson(&part_header, &[sleep(2, 60)])
    };
    let one =
        |stream: &str, window: Value, row: Value| replace_part(stream, BATCH_1, window, &[row]);
    let sleep_with = |member: &str, value: Value| {
        let mut row = sleep(2, 60);
        row["data"][member] = value;
        one("sleepSession", ts_window(&id(100), 1, 10, 1, 1), row)
    };
    let workout_with = |member: &str, value: Value| {
        let mut row = workout(2, "run", 60);
        row["data"][member] = value;
        one("workout", ts_window(&id(100), 1, 10, 1, 1), row)
    };
    let journal_with = |member: &str, value: Option<Value>| {
        let mut row = journal(2, "coffee", true);
        match value {
            Some(value) => row["data"][member] = value,
            None => {
                row["data"].as_object_mut().unwrap().remove(member);
            }
        }
        one("journal", day_window(&id(100), 1, 10, 1, 1), row)
    };

    let cases: Vec<(&str, Vec<u8>, &str)> = vec![
        (
            "append delivery for a mutable stream",
            daily_part(&|h| h["delivery"] = json!("append")),
            "invalid_header",
        ),
        (
            "missing window",
            daily_part(&|h| {
                h.as_object_mut().unwrap().remove("window");
            }),
            "invalid_window",
        ),
        (
            "null window",
            daily_part(&|h| h["window"] = Value::Null),
            "invalid_window",
        ),
        (
            "wrong selector",
            daily_part(&|h| h["window"]["selector"] = json!("startTs")),
            "invalid_window",
        ),
        (
            "unpadded day bound",
            daily_part(&|h| h["window"]["startInclusive"] = json!("2026-8-1")),
            "invalid_window",
        ),
        (
            "impossible day bound",
            daily_part(&|h| h["window"]["endExclusive"] = json!("2026-02-30")),
            "invalid_window",
        ),
        (
            "timestamp bound on a day selector",
            daily_part(&|h| h["window"]["startInclusive"] = json!(1)),
            "invalid_window",
        ),
        (
            "empty day window",
            daily_part(&|h| h["window"]["endExclusive"] = json!(day(1))),
            "invalid_window",
        ),
        (
            "reversed day window",
            daily_part(&|h| h["window"]["startInclusive"] = json!(day(11))),
            "invalid_window",
        ),
        (
            "string timestamp bound",
            sleep_part(&|h| h["window"]["startInclusive"] = json!(ts(1).to_string())),
            "invalid_window",
        ),
        (
            "fractional timestamp bound",
            sleep_part(&|h| h["window"]["endExclusive"] = json!(ts(10) as f64 + 0.5)),
            "invalid_window",
        ),
        (
            "empty timestamp window",
            sleep_part(&|h| h["window"]["endExclusive"] = json!(ts(1))),
            "invalid_window",
        ),
        (
            "part zero",
            daily_part(&|h| h["window"]["part"] = json!(0)),
            "invalid_window",
        ),
        (
            "part beyond parts",
            daily_part(&|h| h["window"]["part"] = json!(2)),
            "invalid_window",
        ),
        (
            "zero parts",
            daily_part(&|h| h["window"]["parts"] = json!(0)),
            "invalid_window",
        ),
        (
            "uppercase replacementId",
            daily_part(&|h| {
                h["window"]["replacementId"] = json!("BF8B735E-F157-4B35-BEB2-9B086D10D5BD");
            }),
            "invalid_window",
        ),
        (
            "missing part",
            daily_part(&|h| {
                h["window"].as_object_mut().unwrap().remove("part");
            }),
            "invalid_window",
        ),
        (
            "non-null startCursor",
            daily_part(&|h| h["startCursor"] = cursor(1, START_SHA)),
            "invalid_cursor",
        ),
        (
            "non-null endCursor",
            daily_part(&|h| h["endCursor"] = cursor(1, END_SHA)),
            "invalid_cursor",
        ),
        (
            "missing endCursor",
            daily_part(&|h| {
                h.as_object_mut().unwrap().remove("endCursor");
            }),
            "invalid_header",
        ),
        (
            "empty part of a multi-part replacement",
            replace_part(
                "dailyMetric",
                BATCH_1,
                day_window(&id(100), 1, 10, 1, 2),
                &[],
            ),
            "empty_batch",
        ),
        (
            "day key at the exclusive end",
            one(
                "dailyMetric",
                day_window(&id(100), 1, 10, 1, 1),
                daily(10, 1.0),
            ),
            "record_outside_window",
        ),
        (
            "timestamp key before the window",
            one(
                "sleepSession",
                ts_window(&id(100), 2, 10, 1, 1),
                sleep(1, 60),
            ),
            "record_outside_window",
        ),
        (
            "malformed day key",
            one(
                "journal",
                day_window(&id(100), 1, 10, 1, 1),
                record(
                    json!({"day": "2026-08-3", "question": "q"}),
                    json!({"answeredYes": true, "notes": null, "numericValue": null}),
                ),
            ),
            "invalid_record",
        ),
        (
            "null sleepSession.endTs",
            sleep_with("endTs", Value::Null),
            "invalid_record",
        ),
        (
            "null sleepSession.userEdited",
            sleep_with("userEdited", Value::Null),
            "invalid_record",
        ),
        (
            "integer sleepSession.userEdited",
            sleep_with("userEdited", json!(0)),
            "invalid_record",
        ),
        (
            "fractional sleepSession.endTs",
            sleep_with("endTs", json!(1.5)),
            "invalid_record",
        ),
        (
            "null workout.endTs",
            workout_with("endTs", Value::Null),
            "invalid_record",
        ),
        (
            "null workout.source",
            workout_with("source", Value::Null),
            "invalid_record",
        ),
        (
            "missing journal.answeredYes",
            journal_with("answeredYes", None),
            "invalid_record",
        ),
        (
            "null journal.answeredYes",
            journal_with("answeredYes", Some(Value::Null)),
            "invalid_record",
        ),
        (
            "duplicate key within a part",
            replace_part(
                "dailyMetric",
                BATCH_1,
                day_window(&id(100), 1, 10, 1, 1),
                &[daily(2, 1.0), daily(2, 2.0)],
            ),
            "duplicate_key",
        ),
    ];

    for (name, entity, code) in cases {
        let db = memory_push_db().await;
        let before = replace_state(&db).await;
        let reply = post(&db, &entity, true).await;
        assert_eq!(reply.json["code"], code, "{name}");
        assert_error(&reply, StatusCode::UNPROCESSABLE_ENTITY, code);
        assert_eq!(replace_state(&db).await, before, "{name}");
    }
}

#[tokio::test]
async fn duplicate_key_across_parts_is_422_and_leaves_the_replacement_incomplete() {
    let db = memory_push_db().await;
    let part = |n: u32| {
        replace_part(
            "dailyMetric",
            &id(u64::from(n)),
            day_window(&id(100), 1, 10, n, 2),
            &[daily(2, f64::from(n))],
        )
    };
    assert_eq!(post(&db, &part(1), true).await.status, StatusCode::OK);
    let before = replace_state(&db).await;

    let reply = post(&db, &part(2), true).await;

    assert_error(&reply, StatusCode::UNPROCESSABLE_ENTITY, "duplicate_key");
    assert_eq!(replace_state(&db).await, before);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM daily_metric").await, 0);
}

#[tokio::test]
async fn rotating_receiver_state_id_discards_staging_and_generation_fences() {
    let db = memory_push_db().await;
    let seed = replace_part(
        "journal",
        &id(4),
        day_window(&id(102), 1, 15, 1, 1),
        &[journal(12, "coffee", true)],
    );
    assert_eq!(post(&db, &seed, true).await.status, StatusCode::OK);
    assert_eq!(
        post(&db, &hr_batch(BATCH_1, &[hr(1, 61)]), true)
            .await
            .status,
        StatusCode::OK
    );
    let old_part_1 = replace_part(
        "journal",
        &id(1),
        day_window(&id(100), 1, 10, 1, 2),
        &[journal(2, "coffee", true)],
    );
    let old_part_2 = replace_part(
        "journal",
        &id(2),
        day_window(&id(100), 1, 10, 2, 2),
        &[journal(3, "coffee", true)],
    );
    assert_eq!(post(&db, &old_part_1, true).await.status, StatusCode::OK);
    let newer = replace_part("journal", &id(3), day_window(&id(101), 1, 10, 1, 1), &[]);
    assert_eq!(post(&db, &newer, true).await.status, StatusCode::OK);
    assert_eq!(
        post(&db, &old_part_2, true).await.status,
        StatusCode::CONFLICT
    );

    let pending = replace_part(
        "journal",
        &id(5),
        day_window(&id(103), 1, 10, 1, 2),
        &[journal(4, "coffee", true)],
    );
    assert_eq!(post(&db, &pending, true).await.status, StatusCode::OK);
    let before_rotation = replace_state(&db).await;
    sqlx::query("UPDATE receiver_state SET receiver_state_id = receiver_state_id WHERE id = 1")
        .execute(&db)
        .await
        .unwrap();
    assert_eq!(
        replace_state(&db).await,
        before_rotation,
        "unchanged ID preserves state"
    );

    let mut tx = db.begin().await.unwrap();
    sqlx::query("UPDATE receiver_state SET receiver_state_id = ? WHERE id = 1")
        .bind(uuid::Uuid::new_v4().hyphenated().to_string())
        .execute(&mut *tx)
        .await
        .unwrap();
    let metadata_counts = [
        "SELECT COUNT(*) FROM batch_ledger",
        "SELECT COUNT(*) FROM replacement",
        "SELECT COUNT(*) FROM replacement_scope",
        "SELECT COUNT(*) FROM replacement_part",
    ];
    for sql in metadata_counts {
        assert_eq!(
            sqlx::query_scalar::<_, i64>(sql)
                .fetch_one(&mut *tx)
                .await
                .unwrap(),
            0,
            "{sql}"
        );
    }
    tx.rollback().await.unwrap();
    assert_eq!(
        replace_state(&db).await,
        before_rotation,
        "rotation rollback preserves metadata"
    );

    sqlx::query("UPDATE receiver_state SET receiver_state_id = ? WHERE id = 1")
        .bind(uuid::Uuid::new_v4().hyphenated().to_string())
        .execute(&db)
        .await
        .unwrap();
    for sql in metadata_counts {
        assert_eq!(count(&db, sql).await, 0, "{sql}");
    }
    assert_eq!(
        count(&db, "SELECT COUNT(*) FROM journal").await,
        1,
        "health rows survive rotation"
    );
    assert_eq!(
        count(&db, "SELECT bpm FROM hr_sample WHERE ts = 1").await,
        61
    );

    // The new generation has no fence and no staging: the old parts form a fresh replacement.
    assert_eq!(post(&db, &old_part_2, true).await.status, StatusCode::OK);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM journal").await, 1);
    assert_eq!(post(&db, &old_part_1, true).await.status, StatusCode::OK);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM journal").await, 3);
}
