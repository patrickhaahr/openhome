use crate::mirror::Mirror;
use crate::mirror::Window;
use crate::send_request::send_request;
use axum::http::StatusCode;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;

pub(crate) const API_KEY: &str = "test-api-key";

pub(crate) const DERIVED: &str = "openhome-derived";

pub(crate) const SOURCE: &str = "81906e30-187d-4546-8f8a-9949b82d62fa";

pub(crate) const IMPORTED: &str = "my-whoop";

pub(crate) const COMPUTED: &str = "my-whoop-noop";

pub(crate) const REAL_PUSH_AT: &str = "2026-09-26T01:51:38.318Z";

pub(crate) const REAL_DAYS: Window = Window::Days("2026-09-13", "2026-09-27");

pub(crate) const REAL_STARTS: Window = Window::Starts(1_789_250_400, 1_790_460_000);

pub(crate) const SEP_20: i64 = 1_789_855_200;

impl Mirror {
    pub(crate) async fn recovery(&self, day: &str, api_key: Option<&str>) -> (StatusCode, Value) {
        let app = crate::mirror::test_app_with_noop_db(self.db.clone()).await;
        send_request(app, &format!("/api/training/days/{day}/recovery"), api_key).await
    }

    pub(crate) async fn recovery_day(&self, day: &str) -> Value {
        let (status, body) = self.recovery(day, Some(API_KEY)).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        body
    }
}

pub(crate) fn daily(day: &str, fields: Value) -> Value {
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

pub(crate) fn sleep(start_ts: i64, end_ts: i64, fields: Value) -> Value {
    let mut data = json!({
        "endTs": end_ts, "efficiency": null, "restingHr": null, "avgHrv": null,
        "stagesJSON": null, "userEdited": false, "startTsAdjusted": null, "motionJSON": null,
        "sleepStateJSON": null, "stagingSparse": null,
    });
    overlay(&mut data, fields);
    json!({"type": "record", "key": {"startTs": start_ts}, "data": data})
}

pub(crate) fn journal(day: &str, question: &str, answered_yes: bool, fields: Value) -> Value {
    let mut data = json!({"answeredYes": answered_yes, "notes": null, "numericValue": null});
    overlay(&mut data, fields);
    json!({"type": "record", "key": {"day": day, "question": question}, "data": data})
}

pub(crate) fn overlay(data: &mut Value, fields: Value) {
    let Value::Object(fields) = fields else {
        panic!("fields must be a JSON object");
    };
    for (name, value) in fields {
        assert!(data.get(&name).is_some(), "unknown field {name}");
        data[&name] = value;
    }
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "JSON builders take `json!` literals by value, as `json!` itself does"
)]
pub(crate) fn measured(value: Value, unit: &str, source: &str) -> Value {
    json!({"value": value, "unit": unit, "source": source})
}

pub(crate) fn absent(unit: &str) -> Value {
    json!({"value": null, "unit": unit, "source": null})
}

pub(crate) const NIGHT_A_STAGES: &str = r#"[{"end":1789423980,"stage":"deep","start":1789423080},{"end":1789424430,"stage":"light","start":1789423980},{"end":1789424790,"stage":"deep","start":1789424430},{"end":1789425990,"stage":"light","start":1789424790},{"end":1789427010,"stage":"rem","start":1789425990},{"end":1789428330,"stage":"light","start":1789427010},{"end":1789428886,"stage":"deep","start":1789428330}]"#;

pub(crate) const NIGHT_B_STAGES: &str = r#"[{"start":1789428180,"end":1789430370,"stage":"deep"},{"start":1789430370,"end":1789430670,"stage":"light"},{"start":1789430670,"end":1789432470,"stage":"rem"},{"start":1789432470,"end":1789432890,"stage":"light"},{"start":1789432890,"end":1789433280,"stage":"rem"},{"start":1789433280,"end":1789433850,"stage":"light"},{"start":1789433850,"end":1789434150,"stage":"rem"},{"start":1789434150,"end":1789434315,"stage":"light"},{"start":1789434315,"end":1789455600,"stage":"wake"}]"#;

pub(crate) fn night_a() -> Value {
    sleep(
        1_789_423_080,
        1_789_428_886,
        json!({
            "efficiency": 1.0, "restingHr": 47.0, "avgHrv": 88.2272585751,
            "stagesJSON": NIGHT_A_STAGES, "stagingSparse": false,
        }),
    )
}

pub(crate) fn night_b(user_edited: bool) -> Value {
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

pub(crate) fn real_imported_days() -> Vec<Value> {
    vec![daily("2026-09-14", json!({"strain": 3.1}))]
}

pub(crate) fn real_computed_days() -> Vec<Value> {
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

pub(crate) async fn real_mirror() -> Mirror {
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

pub(crate) async fn covered_mirror(
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
