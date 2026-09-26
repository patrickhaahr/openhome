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
            "mutable stream",
            with(&|h| h["stream"] = json!("dailyMetric")),
            "unsupported_stream",
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
