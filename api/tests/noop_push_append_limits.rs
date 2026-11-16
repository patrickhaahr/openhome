//! NOOP push append streams: body size bounds, request headers and concurrent posts.

mod common;
mod noop_push_support;

use axum::http::StatusCode;
use noop_push_support::{
    API_KEY, BATCH_1, END_SHA, PUSH_TOKEN, assert_error, bearer, count, cursor, expected_ack, gzip,
    header, hr, hr_batch, memory_push_db, ndjson, post, push_app, record, rows, send,
};
use openhome_api::services::noop_push;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;

const MAX_DECODED: usize = 4 * 1024 * 1024;

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

/// Concurrent deliveries of one `batchId` with differing bytes: `BEGIN IMMEDIATE` serializes on
/// the ledger, so exactly one applies and the other settles as a `batch_conflict`. The winning
/// bytes and their encodings depend on scheduling, so the test asserts the state, then replays
/// both deliveries against the settled ledger.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_differing_bytes_for_one_batch_id_apply_exactly_once() {
    let path = std::env::temp_dir().join(format!("noop-push-conflict-{}.db", uuid::Uuid::new_v4()));
    let db = noop_push::connect(&format!("sqlite:{}", path.display()))
        .await
        .unwrap();
    let entities = [61, 99].map(|bpm| hr_batch(BATCH_1, &[hr(1, bpm)]));

    let (first, second) = tokio::join!(
        tokio::spawn({
            let (db, entity) = (db.clone(), entities[0].clone());
            async move { post(&db, &entity, true).await }
        }),
        tokio::spawn({
            let (db, entity) = (db.clone(), entities[1].clone());
            async move { post(&db, &entity, false).await }
        }),
    );
    let replies = [first.unwrap(), second.unwrap()];
    let statuses: Vec<StatusCode> = replies.iter().map(|reply| reply.status).collect();

    let ok = replies
        .iter()
        .filter(|reply| reply.status == StatusCode::OK)
        .collect::<Vec<_>>();
    let conflict = replies
        .iter()
        .find(|reply| reply.status == StatusCode::CONFLICT)
        .expect("exactly one of the two conflicting deliveries is accepted");
    assert_eq!(ok.len(), 1, "{:?}", statuses);
    assert_error(conflict, StatusCode::CONFLICT, "batch_conflict");
    assert_eq!(
        ok[0].json,
        expected_ack("hrSample", BATCH_1, cursor(101, END_SHA), 1)
    );
    let accepted_ack = ok[0].raw.clone();
    assert_eq!(count(&db, "SELECT COUNT(*) FROM hr_sample").await, 1);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM batch_ledger").await, 1);

    let stored_bpm = count(&db, "SELECT bpm FROM hr_sample").await;
    assert!(
        stored_bpm == 61 || stored_bpm == 99,
        "the applied delivery is stored: {stored_bpm}"
    );
    let accepted_entity = &entities[usize::from(stored_bpm != 61)];
    let rejected_entity = &entities[usize::from(stored_bpm == 61)];
    let replay = post(&db, accepted_entity, false).await;
    assert_eq!(replay.raw, accepted_ack);
    assert_error(
        &post(&db, rejected_entity, true).await,
        StatusCode::CONFLICT,
        "batch_conflict",
    );
    assert_eq!(count(&db, "SELECT COUNT(*) FROM batch_ledger").await, 1);

    db.close().await;
    for suffix in ["", "-wal", "-shm", "-journal"] {
        let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
    }
}
