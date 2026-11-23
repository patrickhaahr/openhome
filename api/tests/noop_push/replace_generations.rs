use crate::support::{
    BATCH_1, assert_error, assert_ok, count, daily, daily_recoveries, day_window, hr, hr_batch, id,
    journal, memory_push_db, ndjson, post, recovery_rows, replace_ack, replace_header,
    replace_part, replace_state, ts_window, workout,
};
use axum::http::StatusCode;
use pretty_assertions::assert_eq;
use serde_json::json;

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

    assert_eq!(post(&db, &old_part_2, true).await.status, StatusCode::OK);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM journal").await, 1);
    assert_eq!(post(&db, &old_part_1, true).await.status, StatusCode::OK);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM journal").await, 3);
}
