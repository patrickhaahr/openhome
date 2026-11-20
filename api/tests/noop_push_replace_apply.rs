mod common;
mod noop_push_support;

use axum::http::StatusCode;
use axum::http::header;
use noop_push_support::{
    BATCH_1, DEVICE, SOURCE_A, SOURCE_B, assert_ok, count, daily, daily_recoveries, day,
    day_window, id, journal, memory_push_db, ndjson, post, record, recovery_rows, replace_ack,
    replace_header, replace_part, rows, sleep, ts, ts_window, workout,
};
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;

#[tokio::test]
async fn stores_every_mutable_stream_columnar_and_acks_with_null_end_cursor() {
    let cases = [
        (
            "dailyMetric",
            day_window(&id(1000), 1, 15, 1, 1),
            record(
                json!({"day": "2026-08-05"}),
                json!({
                    "totalSleepMin": 420.5, "efficiency": 0.91, "deepMin": 80.0, "remMin": 95.5,
                    "lightMin": 245.0, "disturbances": 3, "restingHr": 52, "avgHrv": 61.2,
                    "recovery": 77.0, "strain": 12.4, "exerciseCount": 1, "spo2Pct": 96.5,
                    "skinTempDevC": -0.3, "respRateBpm": 14.8, "steps": 9001,
                    "activeKcalEst": 512.5, "spo2Red": 1200.5, "spo2Ir": 1300,
                }),
            ),
            "SELECT json_object('source_id', source_id, 'device_id', device_id, 'day', day, \
             'total_sleep_min', total_sleep_min, 'efficiency', efficiency, 'deep_min', deep_min, \
             'rem_min', rem_min, 'light_min', light_min, 'disturbances', disturbances, \
             'resting_hr', resting_hr, 'avg_hrv', avg_hrv, 'recovery', recovery, \
             'strain', strain, 'exercise_count', exercise_count, 'spo2_pct', spo2_pct, \
             'skin_temp_dev_c', skin_temp_dev_c, 'resp_rate_bpm', resp_rate_bpm, \
             'steps', steps, 'active_kcal_est', active_kcal_est, 'spo2_red', spo2_red, \
             'spo2_ir', spo2_ir) FROM daily_metric",
            json!({
                "source_id": SOURCE_A, "device_id": DEVICE, "day": "2026-08-05",
                "total_sleep_min": 420.5, "efficiency": 0.91, "deep_min": 80.0,
                "rem_min": 95.5, "light_min": 245.0, "disturbances": 3, "resting_hr": 52.0,
                "avg_hrv": 61.2, "recovery": 77.0, "strain": 12.4, "exercise_count": 1,
                "spo2_pct": 96.5, "skin_temp_dev_c": -0.3, "resp_rate_bpm": 14.8,
                "steps": 9001, "active_kcal_est": 512.5, "spo2_red": 1200.5, "spo2_ir": 1300.0,
            }),
        ),
        (
            "sleepSession",
            ts_window(&id(1001), 1, 15, 1, 1),
            record(
                json!({"startTs": ts(5)}),
                json!({
                    "endTs": ts(5) + 28_800, "efficiency": 0.88, "restingHr": 50.5,
                    "avgHrv": 70, "stagesJSON": "[{\"stage\":\"deep\"}]", "userEdited": true,
                    "startTsAdjusted": ts(5) - 60, "motionJSON": "{\"m\":1}",
                    "sleepStateJSON": "[]", "stagingSparse": false,
                }),
            ),
            "SELECT json_object('source_id', source_id, 'device_id', device_id, \
             'start_ts', start_ts, 'end_ts', end_ts, 'efficiency', efficiency, \
             'resting_hr', resting_hr, 'avg_hrv', avg_hrv, 'stages_json', stages_json, \
             'user_edited', user_edited, 'start_ts_adjusted', start_ts_adjusted, \
             'motion_json', motion_json, 'sleep_state_json', sleep_state_json, \
             'staging_sparse', staging_sparse) FROM sleep_session",
            json!({
                "source_id": SOURCE_A, "device_id": DEVICE, "start_ts": ts(5),
                "end_ts": ts(5) + 28_800, "efficiency": 0.88, "resting_hr": 50.5,
                "avg_hrv": 70.0, "stages_json": "[{\"stage\":\"deep\"}]", "user_edited": 1,
                "start_ts_adjusted": ts(5) - 60, "motion_json": "{\"m\":1}",
                "sleep_state_json": "[]", "staging_sparse": 0,
            }),
        ),
        (
            "workout",
            ts_window(&id(1002), 1, 15, 1, 1),
            record(
                json!({"startTs": ts(5), "sport": "running"}),
                json!({
                    "endTs": ts(5) + 3600, "source": "healthConnect", "durationS": 3600,
                    "energyKcal": 640.5, "avgHr": 151, "maxHr": 182.5, "strain": 14.1,
                    "distanceM": 10012.5, "zonesJSON": "{\"z2\":0.4}", "notes": "tempo",
                    "routePolyline": "_p~iF~ps|U", "steps": 9500,
                }),
            ),
            "SELECT json_object('source_id', source_id, 'device_id', device_id, \
             'start_ts', start_ts, 'sport', sport, 'end_ts', end_ts, 'source', source, \
             'duration_s', duration_s, 'energy_kcal', energy_kcal, 'avg_hr', avg_hr, \
             'max_hr', max_hr, 'strain', strain, 'distance_m', distance_m, \
             'zones_json', zones_json, 'notes', notes, 'route_polyline', route_polyline, \
             'steps', steps) FROM workout",
            json!({
                "source_id": SOURCE_A, "device_id": DEVICE, "start_ts": ts(5),
                "sport": "running", "end_ts": ts(5) + 3600, "source": "healthConnect",
                "duration_s": 3600.0, "energy_kcal": 640.5, "avg_hr": 151.0, "max_hr": 182.5,
                "strain": 14.1, "distance_m": 10012.5, "zones_json": "{\"z2\":0.4}",
                "notes": "tempo", "route_polyline": "_p~iF~ps|U", "steps": 9500,
            }),
        ),
        (
            "journal",
            day_window(&id(1003), 1, 15, 1, 1),
            record(
                json!({"day": "2026-08-05", "question": "Caffeine after 2pm?"}),
                json!({"answeredYes": true, "notes": "one espresso", "numericValue": 1.5}),
            ),
            "SELECT json_object('source_id', source_id, 'device_id', device_id, 'day', day, \
             'question', question, 'answered_yes', answered_yes, 'notes', notes, \
             'numeric_value', numeric_value) FROM journal",
            json!({
                "source_id": SOURCE_A, "device_id": DEVICE, "day": "2026-08-05",
                "question": "Caffeine after 2pm?", "answered_yes": 1, "notes": "one espresso",
                "numeric_value": 1.5,
            }),
        ),
    ];

    for (stream, window, row, select, expected) in cases {
        let db = memory_push_db().await;
        let reply = post(&db, &replace_part(stream, BATCH_1, window, &[row]), true).await;

        assert_ok(&reply, &replace_ack(stream, BATCH_1, 1));
        assert_eq!(
            reply.headers[header::CONTENT_TYPE],
            "application/json",
            "{stream}"
        );
        assert_eq!(rows(&db, select).await, vec![expected], "{stream}");
    }
}

#[tokio::test]
async fn each_mutable_stream_replaces_its_window_and_leaves_rows_outside_untouched() {
    type Build = fn(i64, i64) -> Value;
    type Window = fn(&str, i64, i64, u32, u32) -> Value;
    let cases: [(&str, Window, Build, &str); 4] = [
        (
            "dailyMetric",
            day_window,
            |index, variant| daily(index, variant as f64),
            "SELECT json_object('k', day, 'v', recovery) FROM daily_metric ORDER BY day",
        ),
        (
            "sleepSession",
            ts_window,
            |index, variant| sleep(index, variant),
            "SELECT json_object('k', start_ts, 'v', end_ts - start_ts) FROM sleep_session \
             ORDER BY start_ts",
        ),
        (
            "workout",
            ts_window,
            |index, variant| workout(index, "run", variant),
            "SELECT json_object('k', start_ts, 'v', end_ts - start_ts) FROM workout \
             ORDER BY start_ts",
        ),
        (
            "journal",
            day_window,
            |index, variant| journal(index, "coffee", variant == 2),
            "SELECT json_object('k', day, 'v', answered_yes + 1) FROM journal ORDER BY day",
        ),
    ];

    for (stream, window, build, select) in cases {
        let db = memory_push_db().await;
        let key = |index: i64| match stream {
            "dailyMetric" | "journal" => json!(day(index)),
            _ => json!(ts(index)),
        };
        let seed: Vec<Value> = [1, 3, 5, 8].map(|index| build(index, 1)).to_vec();
        let reply = post(
            &db,
            &replace_part(stream, &id(1), window(&id(100), 1, 10, 1, 1), &seed),
            true,
        )
        .await;
        assert_ok(&reply, &replace_ack(stream, &id(1), 4));

        let replacement = [build(3, 2), build(4, 2)];
        let reply = post(
            &db,
            &replace_part(stream, &id(2), window(&id(101), 3, 6, 1, 1), &replacement),
            false,
        )
        .await;
        assert_ok(&reply, &replace_ack(stream, &id(2), 2));

        let expected: Vec<Value> = [(1, 1), (3, 2), (4, 2), (8, 1)]
            .iter()
            .map(|(index, variant)| {
                let v = if stream == "dailyMetric" {
                    json!(f64::from(*variant))
                } else {
                    json!(variant)
                };
                json!({"k": key(*index), "v": v})
            })
            .collect();
        assert_eq!(rows(&db, select).await, expected, "{stream}");
    }
}

#[tokio::test]
async fn absent_keys_are_deleted_per_full_composite_key() {
    let db = memory_push_db().await;
    let seed = [
        workout(3, "run", 60),
        workout(3, "swim", 60),
        workout(4, "row", 60),
    ];
    let reply = post(
        &db,
        &replace_part("workout", &id(1), ts_window(&id(100), 1, 10, 1, 1), &seed),
        true,
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK);
    let seed = [journal(3, "coffee", true), journal(3, "alcohol", false)];
    let reply = post(
        &db,
        &replace_part("journal", &id(2), day_window(&id(101), 1, 10, 1, 1), &seed),
        true,
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK);

    let reply = post(
        &db,
        &replace_part(
            "workout",
            &id(3),
            ts_window(&id(102), 1, 10, 1, 1),
            &[workout(3, "swim", 90), workout(4, "row", 60)],
        ),
        true,
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK);
    let reply = post(
        &db,
        &replace_part(
            "journal",
            &id(4),
            day_window(&id(103), 1, 10, 1, 1),
            &[journal(3, "alcohol", true)],
        ),
        true,
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK);

    assert_eq!(
        rows(
            &db,
            "SELECT json_object('sport', sport, 'len', end_ts - start_ts) FROM workout \
             ORDER BY start_ts, sport"
        )
        .await,
        vec![
            json!({"sport": "swim", "len": 90}),
            json!({"sport": "row", "len": 60}),
        ]
    );
    assert_eq!(
        rows(
            &db,
            "SELECT json_object('question', question, 'yes', answered_yes) FROM journal"
        )
        .await,
        vec![json!({"question": "alcohol", "yes": 1})]
    );
}

#[tokio::test]
async fn empty_window_deletes_every_row_in_scope_and_window_only() {
    let db = memory_push_db().await;
    let seed: Vec<Value> = (1..=5).map(|index| daily(index, 50.0)).collect();
    for (n, (source, device)) in [
        (SOURCE_A, DEVICE),
        (SOURCE_B, DEVICE),
        (SOURCE_A, "other-strap"),
    ]
    .into_iter()
    .enumerate()
    {
        let mut part_header = replace_header(
            "dailyMetric",
            &id(n as u64 + 1),
            seed.len(),
            day_window(&id(100 + n as u64), 1, 6, 1, 1),
        );
        part_header["sourceId"] = json!(source);
        part_header["deviceId"] = json!(device);
        let reply = post(&db, &ndjson(&part_header, &seed), true).await;
        assert_eq!(reply.status, StatusCode::OK, "{:?}", reply.json);
    }

    let empty = replace_part(
        "dailyMetric",
        BATCH_1,
        day_window(&id(200), 2, 5, 1, 1),
        &[],
    );
    let reply = post(&db, &empty, true).await;

    assert_ok(&reply, &replace_ack("dailyMetric", BATCH_1, 0));
    assert_eq!(
        daily_recoveries(&db).await,
        recovery_rows(&[(1, 50.0), (5, 50.0)])
    );
    assert_eq!(count(&db, "SELECT COUNT(*) FROM daily_metric").await, 12);
}
