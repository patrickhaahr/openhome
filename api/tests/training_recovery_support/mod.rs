//! NOOP mirror fixtures shared by the Recovery Day test files.

#![allow(dead_code)]

use crate::common;
use crate::common::mirror::Mirror;
use crate::common::mirror::Window;
use axum::http::StatusCode;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;

pub(super) const API_KEY: &str = "test-api-key";

/// The source of values `OpenHome` derives from NOOP's data.
pub(super) const DERIVED: &str = "openhome-derived";

/// The production installation and NOOP's strap namespaces.
pub(super) const SOURCE: &str = "81906e30-187d-4546-8f8a-9949b82d62fa";

pub(super) const IMPORTED: &str = "my-whoop";

pub(super) const COMPUTED: &str = "my-whoop-noop";

/// The production mirror's latest push: 03:51 local time on 2026-09-26.
pub(super) const REAL_PUSH_AT: &str = "2026-09-26T01:51:38.318Z";

/// NOOP's real 14-day windows ending with 2026-09-26, as days and as local-midnight Unix seconds.
pub(super) const REAL_DAYS: Window = Window::Days("2026-09-13", "2026-09-27");

pub(super) const REAL_STARTS: Window = Window::Starts(1_789_250_400, 1_790_460_000);

/// Local midnight at the start of 2026-09-20 (CEST).
pub(super) const SEP_20: i64 = 1_789_855_200;

impl Mirror {
    pub(super) async fn recovery(&self, day: &str, api_key: Option<&str>) -> (StatusCode, Value) {
        let app = common::test_app_with_noop_db(self.db.clone()).await;
        common::send_request(app, &format!("/api/training/days/{day}/recovery"), api_key).await
    }

    pub(super) async fn recovery_day(&self, day: &str) -> Value {
        let (status, body) = self.recovery(day, Some(API_KEY)).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        body
    }
}

/// A `dailyMetric` record for `day`: every field null except `fields`.
pub(super) fn daily(day: &str, fields: Value) -> Value {
    let mut data = json!({
        "totalSleepMin": null, "efficiency": null, "deepMin": null, "remMin": null,
        "lightMin": null, "disturbances": null, "restingHr": null, "avgHrv": null,
        "recovery": null, "strain": null, "exerciseCount": null, "spo2Pct": null,
        "skinTempDevC": null, "respRateBpm": null, "steps": null, "activeKcalEst": null,
        "spo2Red": null, "spo2Ir": null,
    });
    overlay(&mut data, fields);
    json!({"type": "record", "key": {"day": day}, "data": data})
}

/// A `sleepSession` record from `start_ts` to `end_ts`: unstaged and unedited except `fields`.
pub(super) fn sleep(start_ts: i64, end_ts: i64, fields: Value) -> Value {
    let mut data = json!({
        "endTs": end_ts, "efficiency": null, "restingHr": null, "avgHrv": null,
        "stagesJSON": null, "userEdited": false, "startTsAdjusted": null, "motionJSON": null,
        "sleepStateJSON": null, "stagingSparse": null,
    });
    overlay(&mut data, fields);
    json!({"type": "record", "key": {"startTs": start_ts}, "data": data})
}

/// A `journal` record answering `question` on `day`: no value or notes except `fields`.
pub(super) fn journal(day: &str, question: &str, answered_yes: bool, fields: Value) -> Value {
    let mut data = json!({"answeredYes": answered_yes, "notes": null, "numericValue": null});
    overlay(&mut data, fields);
    json!({"type": "record", "key": {"day": day, "question": question}, "data": data})
}

pub(super) fn overlay(data: &mut Value, fields: Value) {
    let Value::Object(fields) = fields else {
        panic!("fields must be a JSON object");
    };
    for (name, value) in fields {
        assert!(data.get(&name).is_some(), "unknown field {name}");
        data[&name] = value;
    }
}

/// A measurement as the API reports it.
#[expect(
    clippy::needless_pass_by_value,
    reason = "JSON builders take `json!` literals by value, as `json!` itself does"
)]
pub(super) fn measured(value: Value, unit: &str, source: &str) -> Value {
    json!({"value": value, "unit": unit, "source": source})
}

pub(super) fn absent(unit: &str) -> Value {
    json!({"value": null, "unit": unit, "source": null})
}

// Real rows from the production mirror.

pub(super) const NIGHT_A_STAGES: &str = r#"[{"end":1789423980,"stage":"deep","start":1789423080},{"end":1789424430,"stage":"light","start":1789423980},{"end":1789424790,"stage":"deep","start":1789424430},{"end":1789425990,"stage":"light","start":1789424790},{"end":1789427010,"stage":"rem","start":1789425990},{"end":1789428330,"stage":"light","start":1789427010},{"end":1789428886,"stage":"deep","start":1789428330}]"#;

pub(super) const NIGHT_B_STAGES: &str = r#"[{"start":1789428180,"end":1789430370,"stage":"deep"},{"start":1789430370,"end":1789430670,"stage":"light"},{"start":1789430670,"end":1789432470,"stage":"rem"},{"start":1789432470,"end":1789432890,"stage":"light"},{"start":1789432890,"end":1789433280,"stage":"rem"},{"start":1789433280,"end":1789433850,"stage":"light"},{"start":1789433850,"end":1789434150,"stage":"rem"},{"start":1789434150,"end":1789434315,"stage":"light"},{"start":1789434315,"end":1789455600,"stage":"wake"}]"#;

/// Computed session 23:58 → 01:34:46 on the night waking 2026-09-15.
pub(super) fn night_a() -> Value {
    sleep(
        1_789_423_080,
        1_789_428_886,
        json!({
            "efficiency": 1.0, "restingHr": 47.0, "avgHrv": 88.2272585751,
            "stagesJSON": NIGHT_A_STAGES, "stagingSparse": false,
        }),
    )
}

/// Computed session 01:23 → 09:00 on 2026-09-15, edited in NOOP.
pub(super) fn night_b(user_edited: bool) -> Value {
    sleep(
        1_789_428_180,
        1_789_455_600,
        json!({
            "efficiency": 1.0, "restingHr": 45.0, "avgHrv": 103.6740725999,
            "stagesJSON": NIGHT_B_STAGES, "userEdited": user_edited,
            "startTsAdjusted": 1_789_428_180, "stagingSparse": false,
        }),
    )
}

pub(super) fn real_imported_days() -> Vec<Value> {
    vec![daily("2026-09-14", json!({"strain": 3.1}))]
}

pub(super) fn real_computed_days() -> Vec<Value> {
    vec![
        daily(
            "2026-09-14",
            json!({"strain": 0.0, "exerciseCount": 0, "activeKcalEst": 181.2600746296}),
        ),
        daily(
            "2026-09-15",
            json!({
                "totalSleepMin": 102.25, "efficiency": 0.2237417943, "deepMin": 36.5,
                "remMin": 41.5, "lightMin": 24.25, "strain": 25.92, "exerciseCount": 0,
                "activeKcalEst": 1230.7682709043,
            }),
        ),
    ]
}

/// The production mirror's rows around 2026-09-15, pushed as NOOP's real 14-day windows.
pub(super) async fn real_mirror() -> Mirror {
    let mut mirror = Mirror::empty().await;
    let imported_days = real_imported_days();
    let computed_days = real_computed_days();
    mirror
        .push(SOURCE, IMPORTED, REAL_DAYS, &imported_days, REAL_PUSH_AT)
        .await;
    mirror
        .push(SOURCE, COMPUTED, REAL_DAYS, &computed_days, REAL_PUSH_AT)
        .await;
    mirror
        .push(SOURCE, IMPORTED, REAL_STARTS, &[], REAL_PUSH_AT)
        .await;
    mirror
        .push(
            SOURCE,
            COMPUTED,
            REAL_STARTS,
            &[night_a(), night_b(true)],
            REAL_PUSH_AT,
        )
        .await;
    mirror
}

/// Both namespaces pushed with empty windows around 2026-09-20, so single tests add only the rows
/// they are about.
pub(super) async fn covered_mirror(
    imported_days: &[Value],
    computed_days: &[Value],
    imported_sessions: &[Value],
    computed_sessions: &[Value],
) -> Mirror {
    let mut mirror = Mirror::empty().await;
    mirror
        .push(SOURCE, IMPORTED, REAL_DAYS, imported_days, REAL_PUSH_AT)
        .await;
    mirror
        .push(SOURCE, COMPUTED, REAL_DAYS, computed_days, REAL_PUSH_AT)
        .await;
    mirror
        .push(
            SOURCE,
            IMPORTED,
            REAL_STARTS,
            imported_sessions,
            REAL_PUSH_AT,
        )
        .await;
    mirror
        .push(
            SOURCE,
            COMPUTED,
            REAL_STARTS,
            computed_sessions,
            REAL_PUSH_AT,
        )
        .await;
    mirror
}
