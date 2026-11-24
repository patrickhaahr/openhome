use crate::support::{
    BATCH_1, BATCH_2, assert_error, assert_ok, count, daily, daily_recoveries, day_window, id,
    journal, memory_push_db, post, recovery_rows, replace_ack, replace_part, replace_state, sleep,
    ts_window,
};
use axum::http::StatusCode;
use openhome_api::services::noop_push;
use pretty_assertions::assert_eq;
use serde_json::Value;

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
