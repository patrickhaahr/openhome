//! Recent Sleep Nights through the authenticated API and the real NOOP push fixture.

mod common;

use axum::http::StatusCode;
use chrono::{Duration, Utc};
use common::mirror::{Mirror, Window};
use openhome_api::services::training::calendar::{CalendarDay, iso};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

const SOURCE: &str = "81906e30-187d-4546-8f8a-9949b82d62fa";
const IMPORTED: &str = "my-whoop";
const COMPUTED: &str = "my-whoop-noop";

fn daily(day: &str, total: f64) -> Value {
    json!({
        "type": "record", "key": {"day": day}, "data": {
            "totalSleepMin": total, "efficiency": 0.8, "deepMin": 60.0,
            "remMin": null, "lightMin": null, "disturbances": null,
            "restingHr": null, "avgHrv": null, "recovery": null,
            "strain": null, "exerciseCount": null, "spo2Pct": null,
            "skinTempDevC": null, "respRateBpm": null, "steps": null,
            "activeKcalEst": null, "spo2Red": null, "spo2Ir": null
        }
    })
}

fn sleep(start_ts: i64, end_ts: i64, edited: bool) -> Value {
    let stages =
        edited.then(|| json!([{"start":start_ts,"end":end_ts,"stage":"light"}]).to_string());
    json!({
        "type": "record", "key": {"startTs": start_ts}, "data": {
            "endTs": end_ts, "efficiency": null, "restingHr": null,
            "avgHrv": null, "stagesJSON": stages, "userEdited": edited,
            "startTsAdjusted": null, "motionJSON": null,
            "sleepStateJSON": null, "stagingSparse": null
        }
    })
}

async fn recent(mirror: &Mirror, count: &str, key: Option<&str>) -> (StatusCode, Value) {
    let app = common::test_app_with_noop_db(mirror.db.clone()).await;
    common::send_request(app, &format!("/api/training/sleep/recent/{count}"), key).await
}

#[tokio::test]
async fn reports_newest_first_with_explicit_unsynced_and_covered_gaps() {
    let today = CalendarDay::of(Utc::now());
    let yesterday = today.previous();
    let earlier = yesterday.previous();
    let earliest = earlier.previous();
    let start = earlier.previous().start().timestamp();
    let end = today.end().timestamp();
    let window_start = earliest.to_string();
    let window_end = today.next().to_string();
    let mut mirror = Mirror::empty().await;
    let pushed_at = iso(Utc::now());

    // Complete imported data on the older night; an edited computed session wins on today.
    mirror
        .push(
            SOURCE,
            IMPORTED,
            Window::Days(
                Box::leak(window_start.clone().into_boxed_str()),
                Box::leak(window_end.clone().into_boxed_str()),
            ),
            &[
                daily(&earlier.to_string(), 440.0),
                daily(&today.to_string(), 470.0),
            ],
            &pushed_at,
        )
        .await;
    mirror
        .push(
            SOURCE,
            COMPUTED,
            Window::Days(
                Box::leak(window_start.into_boxed_str()),
                Box::leak(window_end.into_boxed_str()),
            ),
            &[daily(&today.to_string(), 455.0)],
            &pushed_at,
        )
        .await;
    let older_start = earlier.start().timestamp() - Duration::hours(2).num_seconds();
    let today_start = today.start().timestamp() - Duration::hours(2).num_seconds();
    mirror
        .push(
            SOURCE,
            IMPORTED,
            Window::Starts(start, end),
            &[
                sleep(older_start, earlier.start().timestamp() + 6 * 3600, false),
                sleep(today_start, today.start().timestamp() + 6 * 3600, false),
            ],
            &pushed_at,
        )
        .await;
    mirror
        .push(
            SOURCE,
            COMPUTED,
            Window::Starts(start, end),
            &[sleep(
                today_start,
                today.start().timestamp() + 6 * 3600,
                true,
            )],
            &pushed_at,
        )
        .await;

    let (status, body) = recent(&mirror, "3", Some("test-api-key")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let nights = body["nights"].as_array().unwrap();
    assert_eq!(nights.len(), 3);
    assert_eq!(nights[0]["wake_day"], today.to_string());
    assert_eq!(nights[1]["wake_day"], yesterday.to_string());
    assert_eq!(nights[2]["wake_day"], earlier.to_string());
    assert_eq!(
        nights[0]["sleep"]["total"],
        json!({"value":455.0,"unit":"min","source":COMPUTED})
    );
    assert_eq!(nights[0]["sleep"]["sessions"][0]["user_edited"], true);
    assert_eq!(
        nights[2]["sleep"]["total"],
        json!({"value":440.0,"unit":"min","source":IMPORTED})
    );
    assert_eq!(
        nights[2]["sleep"]["sessions"][0]["start"],
        iso(earlier.start() - Duration::hours(2))
    );
    assert_eq!(
        nights[1]["sleep"]["total"],
        json!({"value":null,"unit":"min","source":null})
    );
    assert_eq!(nights[1]["sleep"]["sessions"], json!([]));
    assert_eq!(
        nights[1]["noop"]["coverage"],
        json!({"daily_metrics":"covered","sleep_sessions":"covered"})
    );
}

#[tokio::test]
async fn late_sync_changes_an_unknown_gap_to_a_recorded_night() {
    let today = CalendarDay::of(Utc::now());
    let mut mirror = Mirror::empty().await;
    let before = recent(&mirror, "1", Some("test-api-key")).await.1;
    assert_eq!(
        before["nights"][0]["noop"]["coverage"],
        json!({"daily_metrics":"unknown","sleep_sessions":"unknown"})
    );
    assert_eq!(before["nights"][0]["sleep"]["total"]["value"], Value::Null);

    let first = Box::leak(today.previous().to_string().into_boxed_str());
    let last = Box::leak(today.next().to_string().into_boxed_str());
    let accepted = iso(Utc::now());
    for device in [IMPORTED, COMPUTED] {
        let rows = if device == IMPORTED {
            vec![daily(&today.to_string(), 420.0)]
        } else {
            vec![]
        };
        mirror
            .push(SOURCE, device, Window::Days(first, last), &rows, &accepted)
            .await;
        mirror
            .push(
                SOURCE,
                device,
                Window::Starts(
                    today.previous().start().timestamp(),
                    today.end().timestamp(),
                ),
                &[],
                &accepted,
            )
            .await;
    }
    let after = recent(&mirror, "1", Some("test-api-key")).await.1;
    assert_eq!(after["nights"][0]["sleep"]["total"]["value"], 420.0);
    assert_eq!(
        after["nights"][0]["noop"]["coverage"],
        json!({"daily_metrics":"covered","sleep_sessions":"covered"})
    );
}

#[tokio::test]
async fn rejects_invalid_counts_and_requires_authentication() {
    let mirror = Mirror::empty().await;
    for count in ["0", "15", "-1", "abc"] {
        assert_eq!(
            recent(&mirror, count, Some("test-api-key")).await.0,
            StatusCode::BAD_REQUEST
        );
    }
    assert_eq!(recent(&mirror, "1", None).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(
        recent(&mirror, "14", Some("test-api-key")).await.1["nights"]
            .as_array()
            .unwrap()
            .len(),
        14
    );
}
