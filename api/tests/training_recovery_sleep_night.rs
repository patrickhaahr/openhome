mod common;
mod training_recovery_support;

use axum::http::StatusCode;
use common::mirror::Mirror;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use training_recovery_support::{
    API_KEY, COMPUTED, DERIVED, IMPORTED, SOURCE, absent, measured, real_mirror,
};

fn as_parsed(expected: &Value) -> Value {
    serde_json::from_str(&expected.to_string()).unwrap()
}

#[tokio::test]
async fn requires_the_api_key() {
    let mirror = real_mirror().await;
    for api_key in [None, Some("wrong-key")] {
        let (status, body) = mirror.recovery("2026-09-15", api_key).await;

        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(
            body,
            json!({"error": "Missing or invalid API key", "status": 401})
        );
    }
}

#[tokio::test]
async fn rejects_days_that_are_not_canonical_calendar_dates() {
    let mirror = Mirror::empty().await;
    for day in ["2026-9-15", "2026-02-30", "20260915", "yesterday"] {
        let (status, body) = mirror.recovery(day, Some(API_KEY)).await;

        assert_eq!(status, StatusCode::BAD_REQUEST, "{day}");
        assert_eq!(
            body,
            json!({
                "error": format!(
                    "Invalid day '{day}': must be a Europe/Copenhagen calendar date written YYYY-MM-DD"
                ),
                "status": 400,
            })
        );
    }
}

#[tokio::test]
async fn reports_unknown_state_before_any_push() {
    let body = Mirror::empty().await.recovery_day("2026-09-15").await;

    assert_eq!(
        body,
        json!({
            "day": "2026-09-15",
            "time_zone": "Europe/Copenhagen",
            "day_start": "2026-09-15T00:00:00+02:00",
            "day_end": "2026-09-16T00:00:00+02:00",
            "sleep": {
                "total": absent("min"),
                "deep": absent("min"),
                "rem": absent("min"),
                "light": absent("min"),
                "efficiency": absent("fraction"),
                "disturbances": absent("count"),
                "sessions": [],
            },
            "resting_hr": absent("beats/min"),
            "hrv_rmssd": absent("ms"),
            "skin_temp_deviation": absent("degC"),
            "respiratory_rate": absent("breaths/min"),
            "spo2": absent("percent"),
            "steps": absent("count"),
            "derived_scores": {
                "recovery": absent("score_0_100"),
                "strain": absent("score_0_100"),
                "rest": absent("score_0_100"),
            },
            "journal": [],
            "noop": {
                "installation_id": null,
                "imported_device_id": IMPORTED,
                "computed_device_id": COMPUTED,
                "last_push_at": null,
                "freshness": "unknown",
                "coverage": {
                    "daily_metrics": "unknown",
                    "sleep_sessions": "unknown",
                    "journal": "unknown",
                },
            },
        })
    );
}

#[tokio::test]
async fn returns_the_real_edited_sleep_night() {
    let body = real_mirror().await.recovery_day("2026-09-15").await;

    assert_eq!(
        body,
        as_parsed(&json!({
            "day": "2026-09-15",
            "time_zone": "Europe/Copenhagen",
            "day_start": "2026-09-15T00:00:00+02:00",
            "day_end": "2026-09-16T00:00:00+02:00",
            "sleep": {
                "total": measured(json!(102.25), "min", COMPUTED),
                "deep": measured(json!(36.5), "min", COMPUTED),
                "rem": measured(json!(41.5), "min", COMPUTED),
                "light": measured(json!(24.25), "min", COMPUTED),
                "efficiency": measured(json!(0.2237417943), "fraction", COMPUTED),
                "disturbances": absent("count"),
                "sessions": [
                    {
                        "start": "2026-09-14T23:58:00+02:00",
                        "end": "2026-09-15T01:34:46+02:00",
                        "detected_start": "2026-09-14T23:58:00+02:00",
                        "duration_min": 5806.0 / 60.0,
                        "stage_min": {
                            "awake": 0.0,
                            "light": 49.5,
                            "deep": 15.0 + 6.0 + 556.0 / 60.0,
                            "rem": 17.0,
                        },
                        "stages": [
                            {"start": "2026-09-14T23:58:00+02:00", "end": "2026-09-15T00:13:00+02:00", "stage": "deep"},
                            {"start": "2026-09-15T00:13:00+02:00", "end": "2026-09-15T00:20:30+02:00", "stage": "light"},
                            {"start": "2026-09-15T00:20:30+02:00", "end": "2026-09-15T00:26:30+02:00", "stage": "deep"},
                            {"start": "2026-09-15T00:26:30+02:00", "end": "2026-09-15T00:46:30+02:00", "stage": "light"},
                            {"start": "2026-09-15T00:46:30+02:00", "end": "2026-09-15T01:03:30+02:00", "stage": "rem"},
                            {"start": "2026-09-15T01:03:30+02:00", "end": "2026-09-15T01:25:30+02:00", "stage": "light"},
                            {"start": "2026-09-15T01:25:30+02:00", "end": "2026-09-15T01:34:46+02:00", "stage": "deep"},
                        ],
                        "efficiency_fraction": 1.0,
                        "resting_hr_bpm": 47.0,
                        "hrv_rmssd_ms": 88.2272585751,
                        "user_edited": false,
                        "staging_sparse": false,
                        "source": COMPUTED,
                    },
                    {
                        "start": "2026-09-15T01:23:00+02:00",
                        "end": "2026-09-15T09:00:00+02:00",
                        "detected_start": "2026-09-15T01:23:00+02:00",
                        "duration_min": 457.0,
                        "stage_min": {"awake": 354.75, "light": 24.25, "deep": 36.5, "rem": 41.5},
                        "stages": [
                            {"start": "2026-09-15T01:23:00+02:00", "end": "2026-09-15T01:59:30+02:00", "stage": "deep"},
                            {"start": "2026-09-15T01:59:30+02:00", "end": "2026-09-15T02:04:30+02:00", "stage": "light"},
                            {"start": "2026-09-15T02:04:30+02:00", "end": "2026-09-15T02:34:30+02:00", "stage": "rem"},
                            {"start": "2026-09-15T02:34:30+02:00", "end": "2026-09-15T02:41:30+02:00", "stage": "light"},
                            {"start": "2026-09-15T02:41:30+02:00", "end": "2026-09-15T02:48:00+02:00", "stage": "rem"},
                            {"start": "2026-09-15T02:48:00+02:00", "end": "2026-09-15T02:57:30+02:00", "stage": "light"},
                            {"start": "2026-09-15T02:57:30+02:00", "end": "2026-09-15T03:02:30+02:00", "stage": "rem"},
                            {"start": "2026-09-15T03:02:30+02:00", "end": "2026-09-15T03:05:15+02:00", "stage": "light"},
                            {"start": "2026-09-15T03:05:15+02:00", "end": "2026-09-15T09:00:00+02:00", "stage": "awake"},
                        ],
                        "efficiency_fraction": 1.0,
                        "resting_hr_bpm": 45.0,
                        "hrv_rmssd_ms": 103.6740725999,
                        "user_edited": true,
                        "staging_sparse": false,
                        "source": COMPUTED,
                    },
                ],
            },
            "resting_hr": absent("beats/min"),
            "hrv_rmssd": absent("ms"),
            "skin_temp_deviation": absent("degC"),
            "respiratory_rate": absent("breaths/min"),
            "spo2": absent("percent"),
            "steps": absent("count"),
            "derived_scores": {
                "recovery": absent("score_0_100"),
                "strain": measured(json!(25.92), "score_0_100", COMPUTED),
                "rest": measured(json!(40.13), "score_0_100", DERIVED),
            },
            "journal": [],
            "noop": {
                "installation_id": SOURCE,
                "imported_device_id": IMPORTED,
                "computed_device_id": COMPUTED,
                "last_push_at": "2026-09-26T03:51:38+02:00",
                "freshness": "confirmed",
                "coverage": {
                    "daily_metrics": "covered",
                    "sleep_sessions": "covered",
                    "journal": "unknown",
                },
            },
        }))
    );
}
