//! Authenticated trend reads across daily Body Weight and NOOP recovery measurements.

mod common;

use axum::http::StatusCode;
use chrono::Duration;
use common::mirror::{Mirror, Window};
use openhome_api::services::training::CalendarDay;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

const KEY: &str = "test-api-key";
const SOURCE: &str = "81906e30-187d-4546-8f8a-9949b82d62fa";
const IMPORTED: &str = "my-whoop";
const COMPUTED: &str = "my-whoop-noop";

async fn trend(
    app: axum::Router,
    metric: &str,
    from: &str,
    to: &str,
    key: Option<&str>,
) -> (StatusCode, Value) {
    common::send_request(
        app,
        &format!("/api/training/trends/{metric}/{from}/{to}"),
        key,
    )
    .await
}

fn daily(day: &str, fields: Value) -> Value {
    let mut data = json!({
        "totalSleepMin": null, "efficiency": null, "deepMin": null, "remMin": null,
        "lightMin": null, "disturbances": null, "restingHr": null, "avgHrv": null,
        "recovery": null, "strain": null, "exerciseCount": null, "spo2Pct": null,
        "skinTempDevC": null, "respRateBpm": null, "steps": null, "activeKcalEst": null,
        "spo2Red": null, "spo2Ir": null,
    });
    let Value::Object(fields) = fields else {
        panic!("fields must be a JSON object");
    };
    for (field, value) in fields {
        data[&field] = value;
    }
    json!({"type":"record", "key":{"day":day}, "data":data})
}

async fn recovery_mirror() -> Mirror {
    let mut mirror = Mirror::empty().await;
    let window = Window::Days("2026-09-27", "2026-10-06");
    let pushed = "2026-10-06T02:00:00.000Z";
    mirror
        .push(
            SOURCE,
            IMPORTED,
            window,
            &[
                daily(
                    "2026-09-29",
                    json!({"totalSleepMin":420.0,"restingHr":50.0,"avgHrv":60.0}),
                ),
                daily(
                    "2026-10-01",
                    json!({"totalSleepMin":450.0,"restingHr":48.0}),
                ),
            ],
            pushed,
        )
        .await;
    mirror
        .push(
            SOURCE,
            COMPUTED,
            window,
            &[
                daily(
                    "2026-09-29",
                    json!({"totalSleepMin":410.0,"restingHr":47.0,"avgHrv":55.0}),
                ),
                daily(
                    "2026-10-01",
                    json!({"totalSleepMin":460.0,"restingHr":46.0,"avgHrv":70.0}),
                ),
            ],
            pushed,
        )
        .await;
    let starts = Window::Starts(
        CalendarDay::parse("2026-09-26")
            .unwrap()
            .start()
            .timestamp(),
        CalendarDay::parse("2026-10-06")
            .unwrap()
            .start()
            .timestamp(),
    );
    for device in [IMPORTED, COMPUTED] {
        mirror.push(SOURCE, device, starts, &[], pushed).await;
    }
    mirror
}

#[tokio::test]
async fn body_weight_uses_only_daily_records_and_counts_partial_weeks() {
    let (app, state) = common::test_app_with_db().await;
    sqlx::query("INSERT INTO body_weight (date, weight_kg) VALUES ('2026-09-29', 80.0), ('2026-10-01', 79.0)")
        .execute(&state.db).await.unwrap();
    let (status, body) = trend(app, "body_weight", "2026-09-29", "2026-10-02", Some(KEY)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["time_zone"], "Europe/Copenhagen");
    assert_eq!(
        body["daily_points"],
        json!([
            {"day":"2026-09-29","value":80.0,"unit":"kg","source":"body_weight","freshness":null,"coverage":null},
            {"day":"2026-09-30","value":null,"unit":"kg","source":null,"freshness":null,"coverage":null},
            {"day":"2026-10-01","value":79.0,"unit":"kg","source":"body_weight","freshness":null,"coverage":null},
            {"day":"2026-10-02","value":null,"unit":"kg","source":null,"freshness":null,"coverage":null},
        ])
    );
    assert_eq!(
        body["weekly_summaries"],
        json!([{
            "week_start":"2026-09-28","week_end":"2026-10-04",
            "range_start":"2026-09-29","range_end":"2026-10-02",
            "calendar_days":4,"observed_days":2,"mean":79.5,"unit":"kg"
        }])
    );
}

#[tokio::test]
async fn four_noop_metrics_preserve_sources_nulls_and_week_boundaries() {
    let mirror = recovery_mirror().await;
    let app = common::test_app_with_noop_db(mirror.db).await;
    for (metric, unit, first, second, second_source) in [
        ("sleep_duration", "min", 420.0, 450.0, IMPORTED),
        ("resting_heart_rate", "beats/min", 50.0, 48.0, IMPORTED),
        ("hrv", "ms", 60.0, 70.0, COMPUTED),
    ] {
        let (status, body) =
            trend(app.clone(), metric, "2026-09-29", "2026-10-05", Some(KEY)).await;
        assert_eq!(status, StatusCode::OK, "{metric}: {body}");
        assert_eq!(body["daily_points"][0]["value"], first);
        assert_eq!(body["daily_points"][0]["source"], IMPORTED);
        assert_eq!(body["daily_points"][0]["unit"], unit);
        assert_eq!(body["daily_points"][1]["value"], Value::Null);
        assert_eq!(body["daily_points"][1]["coverage"], "covered");
        assert_eq!(body["daily_points"][2]["value"], second);
        assert_eq!(body["daily_points"][2]["source"], second_source);
        assert_eq!(body["weekly_summaries"][0]["observed_days"], 2);
        assert_eq!(body["weekly_summaries"][0]["calendar_days"], 6);
        assert_eq!(body["weekly_summaries"][1]["week_start"], "2026-10-05");
        assert_eq!(body["weekly_summaries"][1]["mean"], Value::Null);
    }
}

#[tokio::test]
async fn edited_sleep_uses_the_computed_night_on_the_25_hour_dst_day() {
    let day = CalendarDay::parse("2026-10-25").unwrap();
    let next = day.next();
    assert_eq!((day.end() - day.start()).num_hours(), 25);
    let mut mirror = Mirror::empty().await;
    let pushed = "2026-10-28T02:00:00.000Z";
    let window = Window::Days("2026-10-24", "2026-10-28");
    mirror
        .push(
            SOURCE,
            IMPORTED,
            window,
            &[
                daily("2026-10-25", json!({"totalSleepMin":400.0})),
                daily("2026-10-26", json!({"totalSleepMin":500.0})),
            ],
            pushed,
        )
        .await;
    mirror
        .push(
            SOURCE,
            COMPUTED,
            window,
            &[
                daily("2026-10-25", json!({"totalSleepMin":430.0})),
                daily("2026-10-26", json!({"totalSleepMin":510.0})),
            ],
            pushed,
        )
        .await;
    let start = (day.start() + Duration::hours(23)).timestamp();
    let end = (day.start() + Duration::minutes(24 * 60 + 30)).timestamp();
    let session = json!({
        "type":"record", "key":{"startTs":start},
        "data":{"endTs":end,"efficiency":null,"restingHr":null,"avgHrv":null,
            "stagesJSON":null,"userEdited":true,"startTsAdjusted":null,"motionJSON":null,
            "sleepStateJSON":null,"stagingSparse":null}
    });
    let starts = Window::Starts(
        day.previous().start().timestamp(),
        next.next().start().timestamp(),
    );
    mirror.push(SOURCE, IMPORTED, starts, &[], pushed).await;
    mirror
        .push(SOURCE, COMPUTED, starts, &[session], pushed)
        .await;

    let app = common::test_app_with_noop_db(mirror.db).await;
    let (status, body) = trend(app, "sleep_duration", "2026-10-25", "2026-10-26", Some(KEY)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["daily_points"][0]["value"], 430.0);
    assert_eq!(body["daily_points"][0]["source"], COMPUTED);
    assert_eq!(body["daily_points"][1]["value"], 500.0);
    assert_eq!(body["daily_points"][1]["source"], IMPORTED);
    assert_eq!(body["weekly_summaries"][0]["week_start"], "2026-10-19");
    assert_eq!(body["weekly_summaries"][1]["week_start"], "2026-10-26");
}

#[tokio::test]
async fn late_push_turns_unknown_coverage_into_a_confirmed_measurement() {
    let mut mirror = Mirror::empty().await;
    let app = common::test_app_with_noop_db(mirror.db.clone()).await;
    let read = || trend(app.clone(), "hrv", "2026-09-29", "2026-09-29", Some(KEY));
    let (status, before) = read().await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(before["daily_points"][0]["value"], Value::Null);
    assert_eq!(before["daily_points"][0]["coverage"], "unknown");
    assert_eq!(before["daily_points"][0]["freshness"], "unknown");

    let window = Window::Days("2026-09-29", "2026-09-30");
    let pushed = "2026-09-30T02:00:00.000Z";
    mirror
        .push(
            SOURCE,
            COMPUTED,
            window,
            &[daily("2026-09-29", json!({"avgHrv":55.0}))],
            pushed,
        )
        .await;
    let (_, partial) = read().await;
    assert_eq!(partial["daily_points"][0]["value"], 55.0);
    assert_eq!(partial["daily_points"][0]["coverage"], "unknown");
    assert_eq!(partial["daily_points"][0]["freshness"], "unknown");

    mirror.push(SOURCE, IMPORTED, window, &[], pushed).await;
    let (_, complete) = read().await;
    assert_eq!(complete["daily_points"][0]["value"], 55.0);
    assert_eq!(complete["daily_points"][0]["source"], COMPUTED);
    assert_eq!(complete["daily_points"][0]["coverage"], "covered");
    assert_eq!(complete["daily_points"][0]["freshness"], "confirmed");
    assert_eq!(complete["weekly_summaries"][0]["observed_days"], 1);
}

#[tokio::test]
async fn rejects_unknown_metrics_invalid_ranges_and_missing_auth() {
    let app = common::test_app().await;
    for metric in ["weight", "strain", "resting_hr"] {
        assert_eq!(
            trend(app.clone(), metric, "2026-09-01", "2026-09-01", Some(KEY))
                .await
                .0,
            StatusCode::BAD_REQUEST
        );
    }
    for (from, to) in [
        ("2026-02-30", "2026-03-01"),
        ("2026-09-02", "2026-09-01"),
        ("2026-06-01", "2026-08-30"),
    ] {
        assert_eq!(
            trend(app.clone(), "body_weight", from, to, Some(KEY))
                .await
                .0,
            StatusCode::BAD_REQUEST
        );
    }
    assert_eq!(
        trend(app.clone(), "body_weight", "2026-09-01", "2026-09-01", None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        trend(app, "body_weight", "2026-06-01", "2026-08-29", Some(KEY))
            .await
            .0,
        StatusCode::OK
    );
}
