//! NOOP push append streams: malformed, corrupt and contract-violating batches.

mod common;
mod noop_push_support;

use axum::http::StatusCode;
use noop_push_support::{
    BATCH_1, DEVICE, END_SHA, PUSH_TOKEN, START_SHA, assert_error, bearer, count, cursor,
    expected_ack, header, hr, hr_batch, memory_push_db, ndjson, post, push_app, record, send,
};
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;

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
