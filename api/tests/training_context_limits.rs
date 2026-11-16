//! Training Context: day placement, block ranges and response caps.

//! Training Context (`GET /api/training/context/{from_day}/{to_day}`): Workouts and Body Weight
//! logged through the fitness API beside NOOP rows pushed through the real push endpoint, on one
//! Copenhagen date axis.

mod common;
mod training_context_support;

use axum::http::StatusCode;
use chrono::Duration;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use training_context_support::{API_KEY, at, context, context_ok, day, mirror, noop_workout, sync};

#[tokio::test]
async fn rejects_a_day_with_too_many_noop_workouts_rather_than_truncating() {
    let workouts: Vec<Value> = (0..25)
        .map(|index| {
            let start = at("2026-09-15", Duration::minutes(30 * index));
            noop_workout(start, start + 600, "Walking", "manual", json!({}))
        })
        .collect();
    let mirror = mirror(
        "2026-09-14",
        "2026-09-17",
        &[],
        &[],
        &workouts,
        "2026-09-17T02:00:00.000Z",
    )
    .await;
    let app = common::test_app_with_noop_db(mirror.db).await;

    let (status, body) = context(&app, "2026-09-14", "2026-09-16", Some(API_KEY)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["status"], 422);
    // Days around it are readable on their own.
    let body = context_ok(&app, "2026-09-16", "2026-09-16").await;
    assert_eq!(body["days"][0]["noop_workouts"], json!([]));
}

#[tokio::test]
async fn places_noop_workouts_on_their_local_day_across_the_25_hour_dst_day() {
    let dst = day("2026-10-25");
    assert_eq!((dst.end() - dst.start()).num_hours(), 25);
    // 23:30 local on the 25th is 24.5 hours after its midnight; 00:15 on the 26th follows.
    let late = at("2026-10-25", Duration::minutes(24 * 60 + 30));
    let early = at("2026-10-26", Duration::minutes(15));
    let mirror = mirror(
        "2026-10-24",
        "2026-10-27",
        &[],
        &[],
        &[
            noop_workout(late, late + 1200, "Running", "manual", json!({})),
            noop_workout(early, early + 1200, "Walking", "manual", json!({})),
        ],
        "2026-10-27T02:00:00.000Z",
    )
    .await;
    let app = common::test_app_with_noop_db(mirror.db).await;

    let body = context_ok(&app, "2026-10-25", "2026-10-26").await;
    let starts = |index: usize| -> Vec<Value> {
        body["days"][index]["noop_workouts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|workout| workout["start"].clone())
            .collect()
    };
    assert_eq!(starts(0), [json!("2026-10-25T23:30:00+01:00")]);
    assert_eq!(starts(1), [json!("2026-10-26T00:15:00+01:00")]);
    assert_eq!(
        body["days"][0]["noop"],
        sync("confirmed", "confirmed", "covered")
    );
}

#[tokio::test]
async fn returns_every_day_of_a_ninety_day_block_and_rejects_longer_ranges() {
    let mirror = mirror(
        "2026-05-31",
        "2026-08-30",
        &[],
        &[],
        &[],
        "2026-08-31T02:00:00.000Z",
    )
    .await;
    let app = common::test_app_with_noop_db(mirror.db).await;
    let body = context_ok(&app, "2026-06-01", "2026-08-29").await;
    let days = body["days"].as_array().unwrap();
    assert_eq!(days.len(), 90);
    assert_eq!(days[0]["day"], "2026-06-01");
    assert_eq!(days[89]["day"], "2026-08-29");
    let covered = sync("confirmed", "confirmed", "covered");
    assert!(days.iter().all(|entry| entry["noop"] == covered));

    for (from, to) in [
        ("2026-06-01", "2026-08-30"),
        ("2026-09-02", "2026-09-01"),
        ("2026-02-30", "2026-03-01"),
        ("2026-9-01", "2026-09-02"),
    ] {
        let (status, body) = context(&app, from, to, Some(API_KEY)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{from}..{to}: {body}");
        assert_eq!(body["status"], 400);
    }
    let (status, body) = context(&app, "2026-06-01", "2026-08-30", Some(API_KEY)).await;
    assert_eq!(
        body["error"], "Training Context must cover 1 to 90 calendar days, with from_day <= to_day",
        "{status}"
    );

    for key in [None, Some("wrong-key")] {
        let (status, _) = context(&app, "2026-09-01", "2026-09-01", key).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }
}

#[tokio::test]
async fn rejects_a_block_whose_response_exceeds_the_byte_limit() {
    let (app, state) = common::test_app_with_db().await;
    // 60 Workouts, well under the entry cap, each with a 10,000-character name: about 600 KB.
    sqlx::query(
        "WITH RECURSIVE n(i) AS (SELECT 0 UNION ALL SELECT i + 1 FROM n WHERE i < 59) \
         INSERT INTO workouts (date, name) \
         SELECT date('2026-06-01', '+' || i || ' days'), replace(hex(zeroblob(5000)), '0', 'x') \
         FROM n",
    )
    .execute(&state.db)
    .await
    .unwrap();

    let (status, body) = context(&app, "2026-06-01", "2026-08-29", Some(API_KEY)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(
        body,
        json!({
            "error": "The Training Context from 2026-06-01 to 2026-08-29 is larger than 524288 \
                      bytes; refusing to return a truncated block",
            "status": 422,
        })
    );

    // A shorter block of the same Workouts fits.
    let body = context_ok(&app, "2026-06-01", "2026-06-30").await;
    assert_eq!(body["days"].as_array().unwrap().len(), 30);
    assert_eq!(
        body["days"][0]["workouts"][0]["name"]
            .as_str()
            .unwrap()
            .len(),
        10_000
    );
}

#[tokio::test]
async fn rejects_a_block_with_too_many_logged_entries_rather_than_truncating() {
    let (app, state) = common::test_app_with_db().await;
    sqlx::query(
        "WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < 1001) \
         INSERT INTO workouts (date, name) SELECT '2026-09-15', 'Empty' FROM n",
    )
    .execute(&state.db)
    .await
    .unwrap();

    let (status, body) = context(&app, "2026-09-01", "2026-09-30", Some(API_KEY)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["status"], 422);

    // The cap is inclusive: dropping one Workout leaves exactly 1,000 entries.
    sqlx::query("DELETE FROM workouts WHERE id = (SELECT MAX(id) FROM workouts)")
        .execute(&state.db)
        .await
        .unwrap();
    let body = context_ok(&app, "2026-09-15", "2026-09-15").await;
    assert_eq!(body["days"][0]["workouts"].as_array().unwrap().len(), 1000);
}
