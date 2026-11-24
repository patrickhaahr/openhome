use crate::support::{
    BATCH_1, BATCH_2, DEVICE, END_SHA, SOURCE_A, SOURCE_B, assert_error, count, cursor,
    expected_ack, header, hr, hr_batch, memory_push_db, ndjson, post, record, rows,
};
use axum::http::StatusCode;
use axum::http::header;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;

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
