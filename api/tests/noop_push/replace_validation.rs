use crate::support::{
    BATCH_1, END_SHA, START_SHA, assert_error, count, cursor, daily, day, day_window, id, journal,
    memory_push_db, ndjson, post, record, replace_header, replace_part, replace_state, sleep, ts,
    ts_window, workout,
};
use axum::http::StatusCode;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;

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
