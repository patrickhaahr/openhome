//! Recovery Day reads (`GET /api/training/days/{day}/recovery`) against a NOOP mirror filled
//! through the real push endpoint, next to the in-memory training log the test app always has.

mod common;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use openhome_api::routes::noop_push::{PUSH_PATH, PushState, router as push_router};
use openhome_api::services::noop_push::PushToken;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use sqlx::SqlitePool;
use tower::ServiceExt;

const API_KEY: &str = "test-api-key";
const PUSH_TOKEN: &str = "test-push-token";

/// The production installation and NOOP's strap namespaces.
const SOURCE: &str = "81906e30-187d-4546-8f8a-9949b82d62fa";
const OTHER_SOURCE: &str = "3a3486dd-5030-4e17-a00d-a781399890f9";
const IMPORTED: &str = "my-whoop";
const COMPUTED: &str = "my-whoop-noop";

/// The production mirror's latest push: 03:51 local time on 2026-09-26.
const REAL_PUSH_AT: &str = "2026-09-26T01:51:38.318Z";

/// NOOP's real 14-day windows ending with 2026-09-26, as days and as local-midnight Unix seconds.
const REAL_DAYS: Window = Window::Days("2026-09-13", "2026-09-27");
const REAL_STARTS: Window = Window::Starts(1_789_250_400, 1_790_460_000);

/// Local midnight at the start of 2026-09-20 (CEST).
const SEP_20: i64 = 1_789_855_200;

#[derive(Clone, Copy)]
enum Window {
    /// A `dailyMetric` window of `YYYY-MM-DD` days.
    Days(&'static str, &'static str),
    /// A `sleepSession` window of Unix seconds.
    Starts(i64, i64),
}

/// A NOOP push mirror that fixtures are pushed into through the push endpoint.
struct Mirror {
    db: SqlitePool,
    ids: u64,
}

impl Mirror {
    async fn empty() -> Self {
        Self {
            db: common::memory_noop_db().await,
            ids: 0,
        }
    }

    fn next_id(&mut self) -> String {
        self.ids += 1;
        format!("00000000-0000-4000-8000-{:012x}", self.ids)
    }

    /// Pushes one complete replacement window, recorded as accepted at `accepted_at`.
    async fn push(
        &mut self,
        source: &str,
        device: &str,
        window: Window,
        records: &[Value],
        accepted_at: &str,
    ) {
        self.push_first_part(source, device, window, records, 1, accepted_at)
            .await;
    }

    /// Pushes part 1 of a `parts`-part replacement; with more than one part it stays staged.
    async fn push_first_part(
        &mut self,
        source: &str,
        device: &str,
        window: Window,
        records: &[Value],
        parts: u32,
        accepted_at: &str,
    ) {
        let (stream, selector, start, end) = match window {
            Window::Days(start, end) => ("dailyMetric", "day", json!(start), json!(end)),
            Window::Starts(start, end) => ("sleepSession", "startTs", json!(start), json!(end)),
        };
        let batch_id = self.next_id();
        let header = json!({
            "type": "batch",
            "protocolVersion": "1.0",
            "batchId": batch_id,
            "sourceId": source,
            "deviceId": device,
            "stream": stream,
            "delivery": "replace_window",
            "recordCount": records.len(),
            "startCursor": null,
            "endCursor": null,
            "window": {
                "replacementId": self.next_id(),
                "selector": selector,
                "startInclusive": start,
                "endExclusive": end,
                "part": 1,
                "parts": parts,
            },
        });
        let mut entity = String::new();
        for line in std::iter::once(&header).chain(records) {
            entity.push_str(&line.to_string());
            entity.push('\n');
        }

        let app = push_router(PushState {
            db: self.db.clone(),
            token: PushToken::new(PUSH_TOKEN.to_string()).unwrap(),
        });
        let request = Request::builder()
            .method(Method::POST)
            .uri(PUSH_PATH)
            .header("authorization", format!("Bearer {PUSH_TOKEN}"))
            .header("content-type", "application/x-ndjson; charset=utf-8")
            .header("accept", "application/json")
            .body(Body::from(entity))
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));

        sqlx::query("UPDATE batch_ledger SET accepted_at = ? WHERE batch_id = ?")
            .bind(accepted_at)
            .bind(&batch_id)
            .execute(&self.db)
            .await
            .unwrap();
    }

    async fn recovery(&self, day: &str, api_key: Option<&str>) -> (StatusCode, Value) {
        let app = common::test_app_with_noop_db(self.db.clone()).await;
        common::send_request(app, &format!("/api/training/days/{day}/recovery"), api_key).await
    }

    async fn recovery_day(&self, day: &str) -> Value {
        let (status, body) = self.recovery(day, Some(API_KEY)).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        body
    }
}

/// A `dailyMetric` record for `day`: every field null except `fields`.
fn daily(day: &str, fields: Value) -> Value {
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
fn sleep(start_ts: i64, end_ts: i64, fields: Value) -> Value {
    let mut data = json!({
        "endTs": end_ts, "efficiency": null, "restingHr": null, "avgHrv": null,
        "stagesJSON": null, "userEdited": false, "startTsAdjusted": null, "motionJSON": null,
        "sleepStateJSON": null, "stagingSparse": null,
    });
    overlay(&mut data, fields);
    json!({"type": "record", "key": {"startTs": start_ts}, "data": data})
}

fn overlay(data: &mut Value, fields: Value) {
    for (name, value) in fields.as_object().unwrap() {
        assert!(data.get(name).is_some(), "unknown field {name}");
        data[name] = value.clone();
    }
}

/// A measurement as the API reports it.
fn measured(value: Value, unit: &str, source: &str) -> Value {
    json!({"value": value, "unit": unit, "source": source})
}

fn absent(unit: &str) -> Value {
    json!({"value": null, "unit": unit, "source": null})
}

/// `expected` after the JSON text round trip the response body takes: serde_json's default float
/// parser may land one ulp away from the exact quotient the test computes.
fn as_parsed(expected: Value) -> Value {
    serde_json::from_str(&expected.to_string()).unwrap()
}

// Real rows from the production mirror.

const NIGHT_A_STAGES: &str = r#"[{"end":1789423980,"stage":"deep","start":1789423080},{"end":1789424430,"stage":"light","start":1789423980},{"end":1789424790,"stage":"deep","start":1789424430},{"end":1789425990,"stage":"light","start":1789424790},{"end":1789427010,"stage":"rem","start":1789425990},{"end":1789428330,"stage":"light","start":1789427010},{"end":1789428886,"stage":"deep","start":1789428330}]"#;
const NIGHT_B_STAGES: &str = r#"[{"start":1789428180,"end":1789430370,"stage":"deep"},{"start":1789430370,"end":1789430670,"stage":"light"},{"start":1789430670,"end":1789432470,"stage":"rem"},{"start":1789432470,"end":1789432890,"stage":"light"},{"start":1789432890,"end":1789433280,"stage":"rem"},{"start":1789433280,"end":1789433850,"stage":"light"},{"start":1789433850,"end":1789434150,"stage":"rem"},{"start":1789434150,"end":1789434315,"stage":"light"},{"start":1789434315,"end":1789455600,"stage":"wake"}]"#;

/// Computed session 23:58 → 01:34:46 on the night waking 2026-09-15.
fn night_a() -> Value {
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
fn night_b(user_edited: bool) -> Value {
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

fn real_imported_days() -> Vec<Value> {
    vec![daily("2026-09-14", json!({"strain": 3.1}))]
}

fn real_computed_days() -> Vec<Value> {
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
async fn real_mirror() -> Mirror {
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
async fn covered_mirror(
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
            "derived_scores": {
                "recovery": absent("score_0_100"),
                "strain": absent("score_0_100"),
            },
            "noop": {
                "installation_id": null,
                "imported_device_id": IMPORTED,
                "computed_device_id": COMPUTED,
                "last_push_at": null,
                "freshness": "unknown",
                "coverage": {"daily_metrics": "unknown", "sleep_sessions": "unknown"},
            },
        })
    );
}

#[tokio::test]
async fn returns_the_real_edited_sleep_night() {
    let body = real_mirror().await.recovery_day("2026-09-15").await;

    assert_eq!(
        body,
        as_parsed(json!({
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
                        // Segments: deep 900 s + 360 s + 556 s, light 450 s + 1200 s + 1320 s,
                        // rem 1020 s.
                        "stage_min": {
                            "awake": 0.0,
                            "light": 49.5,
                            "deep": 15.0 + 6.0 + 556.0 / 60.0,
                            "rem": 17.0,
                        },
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
            "derived_scores": {
                "recovery": absent("score_0_100"),
                "strain": measured(json!(25.92), "score_0_100", COMPUTED),
            },
            "noop": {
                "installation_id": SOURCE,
                "imported_device_id": IMPORTED,
                "computed_device_id": COMPUTED,
                "last_push_at": "2026-09-26T03:51:38+02:00",
                "freshness": "confirmed",
                "coverage": {"daily_metrics": "covered", "sleep_sessions": "covered"},
            },
        }))
    );
}

#[tokio::test]
async fn imported_values_win_field_by_field_and_computed_values_fill_nulls() {
    // Real 2026-09-14: the strap's imported strain beats NOOP's computed 0.0.
    let real = real_mirror().await.recovery_day("2026-09-14").await;
    assert_eq!(
        real["derived_scores"]["strain"],
        measured(json!(3.1), "score_0_100", IMPORTED)
    );
    assert_eq!(real["derived_scores"]["recovery"], absent("score_0_100"));
    assert_eq!(real["sleep"]["total"], absent("min"));

    let mirror = covered_mirror(
        &[daily(
            "2026-09-20",
            json!({"restingHr": 52.0, "avgHrv": null, "recovery": null, "strain": null}),
        )],
        &[daily(
            "2026-09-20",
            json!({"restingHr": 50.0, "avgHrv": 71.5, "recovery": 64.0, "strain": 0.0}),
        )],
        &[],
        &[],
    )
    .await;
    let body = mirror.recovery_day("2026-09-20").await;

    assert_eq!(
        body["resting_hr"],
        measured(json!(52.0), "beats/min", IMPORTED)
    );
    assert_eq!(body["hrv_rmssd"], measured(json!(71.5), "ms", COMPUTED));
    assert_eq!(
        body["derived_scores"],
        json!({
            "recovery": measured(json!(64.0), "score_0_100", COMPUTED),
            // A computed zero is a value, not a gap.
            "strain": measured(json!(0.0), "score_0_100", COMPUTED),
        })
    );
}

#[tokio::test]
async fn an_edited_night_takes_the_whole_computed_sleep_block() {
    let imported_night = daily(
        "2026-09-15",
        json!({
            "totalSleepMin": 480.0, "efficiency": 0.91, "deepMin": 90.0, "remMin": 100.0,
            "lightMin": 250.0, "disturbances": 4, "restingHr": 48.0, "avgHrv": 95.0,
        }),
    );
    let mut imported_days = real_imported_days();
    imported_days.push(imported_night);

    for edited in [true, false] {
        let mirror = covered_mirror(
            &imported_days,
            &real_computed_days(),
            &[],
            &[night_a(), night_b(edited)],
        )
        .await;
        let body = mirror.recovery_day("2026-09-15").await;
        let sleep = &body["sleep"];

        if edited {
            assert_eq!(sleep["total"], measured(json!(102.25), "min", COMPUTED));
            assert_eq!(
                sleep["efficiency"],
                measured(json!(0.2237417943), "fraction", COMPUTED)
            );
            assert_eq!(sleep["deep"], measured(json!(36.5), "min", COMPUTED));
            // The block moves whole: the edit's null replaces the imported count.
            assert_eq!(sleep["disturbances"], absent("count"));
        } else {
            assert_eq!(sleep["total"], measured(json!(480.0), "min", IMPORTED));
            assert_eq!(
                sleep["efficiency"],
                measured(json!(0.91), "fraction", IMPORTED)
            );
            assert_eq!(sleep["deep"], measured(json!(90.0), "min", IMPORTED));
            assert_eq!(sleep["disturbances"], measured(json!(4), "count", IMPORTED));
        }
        // Values outside the sleep block keep imported precedence either way.
        assert_eq!(
            body["resting_hr"],
            measured(json!(48.0), "beats/min", IMPORTED)
        );
        assert_eq!(body["hrv_rmssd"], measured(json!(95.0), "ms", IMPORTED));
    }
}

#[tokio::test]
async fn a_scored_computed_night_replaces_a_bare_imported_sleep_total() {
    let computed = [daily(
        "2026-09-20",
        json!({
            "totalSleepMin": 430.0, "efficiency": 0.93, "deepMin": 80.0, "remMin": 95.0,
            "lightMin": 240.0, "disturbances": 6,
        }),
    )];

    let bare = covered_mirror(
        &[daily("2026-09-20", json!({"totalSleepMin": 455.0}))],
        &computed,
        &[],
        &[],
    )
    .await;
    let sleep = bare.recovery_day("2026-09-20").await["sleep"].clone();
    assert_eq!(sleep["total"], measured(json!(430.0), "min", COMPUTED));
    assert_eq!(
        sleep["efficiency"],
        measured(json!(0.93), "fraction", COMPUTED)
    );

    // An import that carries efficiency is scored, so it keeps precedence and computed values
    // only fill its gaps.
    let scored = covered_mirror(
        &[daily(
            "2026-09-20",
            json!({"totalSleepMin": 455.0, "efficiency": 0.88}),
        )],
        &computed,
        &[],
        &[],
    )
    .await;
    let sleep = scored.recovery_day("2026-09-20").await["sleep"].clone();
    assert_eq!(sleep["total"], measured(json!(455.0), "min", IMPORTED));
    assert_eq!(
        sleep["efficiency"],
        measured(json!(0.88), "fraction", IMPORTED)
    );
    assert_eq!(sleep["deep"], measured(json!(80.0), "min", COMPUTED));
    assert_eq!(sleep["disturbances"], measured(json!(6), "count", COMPUTED));
}

#[tokio::test]
async fn richer_computed_sessions_replace_the_imported_night() {
    // 22:00 → 06:30 on the night waking 2026-09-20, fully staged when `staged`.
    let night = |staged: bool| {
        let stages = r#"[{"start":1789848000,"end":1789863000,"stage":"light"},{"start":1789863000,"end":1789878600,"stage":"deep"}]"#;
        let stages = if staged { json!(stages) } else { Value::Null };
        sleep(1_789_848_000, 1_789_878_600, json!({"stagesJSON": stages}))
    };
    let sources = |body: &Value| -> Vec<Value> {
        body["sleep"]["sessions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|session| session["source"].clone())
            .collect()
    };

    let unstaged_import = covered_mirror(&[], &[], &[night(false)], &[night(true)]).await;
    let body = unstaged_import.recovery_day("2026-09-20").await;
    assert_eq!(sources(&body), [json!(COMPUTED)]);
    assert_eq!(
        body["sleep"]["sessions"][0]["stage_min"],
        json!({"awake": 0.0, "light": 250.0, "deep": 260.0, "rem": 0.0})
    );

    let equally_staged = covered_mirror(&[], &[], &[night(true)], &[night(true)]).await;
    let body = equally_staged.recovery_day("2026-09-20").await;
    assert_eq!(sources(&body), [json!(IMPORTED)]);
}

#[tokio::test]
async fn sleep_nights_are_grouped_by_copenhagen_wake_day() {
    let sessions = [
        // 22:00 → 23:50 on 2026-09-19.
        sleep(1_789_848_000, 1_789_854_600, json!({})),
        // 23:55 on 09-19 → 00:10 on 09-20, still 2026-09-19 in UTC.
        sleep(1_789_854_900, 1_789_855_800, json!({})),
        // 23:00 → 23:59:59 on 09-20.
        sleep(1_789_938_000, 1_789_941_599, json!({})),
        // 23:30 on 09-20 → midnight: the end bound belongs to the next day.
        sleep(1_789_939_800, 1_789_941_600, json!({})),
    ];
    let mirror = covered_mirror(&[], &[], &[], &sessions).await;
    let spans = |body: Value| -> Vec<(Value, Value)> {
        body["sleep"]["sessions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|session| (session["start"].clone(), session["end"].clone()))
            .collect()
    };

    assert_eq!(
        spans(mirror.recovery_day("2026-09-19").await),
        [(
            json!("2026-09-19T22:00:00+02:00"),
            json!("2026-09-19T23:50:00+02:00")
        )]
    );
    assert_eq!(
        spans(mirror.recovery_day("2026-09-20").await),
        [
            (
                json!("2026-09-19T23:55:00+02:00"),
                json!("2026-09-20T00:10:00+02:00")
            ),
            (
                json!("2026-09-20T23:00:00+02:00"),
                json!("2026-09-20T23:59:59+02:00")
            ),
        ]
    );
    assert_eq!(
        spans(mirror.recovery_day("2026-09-21").await),
        [(
            json!("2026-09-20T23:30:00+02:00"),
            json!("2026-09-21T00:00:00+02:00")
        )]
    );
}

#[tokio::test]
async fn day_bounds_and_offsets_follow_daylight_saving() {
    let mut mirror = Mirror::empty().await;
    let pushed_at = "2026-10-28T12:00:00.000Z";
    for device in [IMPORTED, COMPUTED] {
        mirror
            .push(
                SOURCE,
                device,
                Window::Days("2026-03-28", "2026-03-31"),
                &[],
                pushed_at,
            )
            .await;
        mirror
            .push(
                SOURCE,
                device,
                Window::Days("2026-10-24", "2026-10-28"),
                &[],
                pushed_at,
            )
            .await;
    }
    // Local midnights 2026-03-28 → 2026-03-31 and 2026-10-24 → 2026-10-28.
    let spring = [
        // 23:30 CET → 08:00 CEST: 7.5 hours across the skipped hour.
        sleep(1_774_737_000, 1_774_764_000, json!({})),
        // 23:15 → 23:45 CEST: inside the 23-hour day.
        sleep(1_774_818_900, 1_774_820_700, json!({})),
        // 00:05 → 00:15 CEST on 03-30: inside a naive 24-hour day, but on the next local day.
        sleep(1_774_821_900, 1_774_822_500, json!({})),
    ];
    let autumn = [
        // 23:00 CEST → 07:00 CET: 9 hours across the repeated hour.
        sleep(1_792_875_600, 1_792_908_000, json!({})),
        // 23:00 → 23:30 CET: past a naive 24-hour day, but inside the 25-hour one.
        sleep(1_792_965_600, 1_792_967_400, json!({})),
        // 23:35 CET → 00:10 CET on 10-26.
        sleep(1_792_967_700, 1_792_969_800, json!({})),
    ];
    mirror
        .push(
            SOURCE,
            IMPORTED,
            Window::Starts(1_774_652_400, 1_774_908_000),
            &[],
            pushed_at,
        )
        .await;
    mirror
        .push(
            SOURCE,
            COMPUTED,
            Window::Starts(1_774_652_400, 1_774_908_000),
            &spring,
            pushed_at,
        )
        .await;
    mirror
        .push(
            SOURCE,
            IMPORTED,
            Window::Starts(1_792_792_800, 1_793_142_000),
            &[],
            pushed_at,
        )
        .await;
    mirror
        .push(
            SOURCE,
            COMPUTED,
            Window::Starts(1_792_792_800, 1_793_142_000),
            &autumn,
            pushed_at,
        )
        .await;
    let night = |body: &Value| -> Vec<Value> {
        body["sleep"]["sessions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|session| json!([session["start"], session["end"], session["duration_min"]]))
            .collect()
    };

    let spring_day = mirror.recovery_day("2026-03-29").await;
    assert_eq!(spring_day["day_start"], "2026-03-29T00:00:00+01:00");
    assert_eq!(spring_day["day_end"], "2026-03-30T00:00:00+02:00");
    assert_eq!(
        night(&spring_day),
        [
            json!([
                "2026-03-28T23:30:00+01:00",
                "2026-03-29T08:00:00+02:00",
                450.0
            ]),
            json!([
                "2026-03-29T23:15:00+02:00",
                "2026-03-29T23:45:00+02:00",
                30.0
            ]),
        ]
    );
    assert_eq!(
        spring_day["noop"]["coverage"],
        json!({"daily_metrics": "covered", "sleep_sessions": "covered"})
    );
    assert_eq!(
        night(&mirror.recovery_day("2026-03-30").await),
        [json!([
            "2026-03-30T00:05:00+02:00",
            "2026-03-30T00:15:00+02:00",
            10.0
        ])]
    );

    let autumn_day = mirror.recovery_day("2026-10-25").await;
    assert_eq!(autumn_day["day_start"], "2026-10-25T00:00:00+02:00");
    assert_eq!(autumn_day["day_end"], "2026-10-26T00:00:00+01:00");
    assert_eq!(
        night(&autumn_day),
        [
            json!([
                "2026-10-24T23:00:00+02:00",
                "2026-10-25T07:00:00+01:00",
                540.0
            ]),
            json!([
                "2026-10-25T23:00:00+01:00",
                "2026-10-25T23:30:00+01:00",
                30.0
            ]),
        ]
    );
    assert_eq!(
        autumn_day["noop"]["coverage"],
        json!({"daily_metrics": "covered", "sleep_sessions": "covered"})
    );
    assert_eq!(
        night(&mirror.recovery_day("2026-10-26").await),
        [json!([
            "2026-10-25T23:35:00+01:00",
            "2026-10-26T00:10:00+01:00",
            35.0
        ])]
    );
    assert_eq!(
        autumn_day["noop"]["last_push_at"],
        "2026-10-28T13:00:00+01:00"
    );
}

#[tokio::test]
async fn freshness_follows_the_older_namespace_push() {
    let mut mirror = real_mirror().await;
    let freshness = |body: &Value| body["noop"]["freshness"].clone();

    assert_eq!(
        freshness(&mirror.recovery_day("2026-09-25").await),
        "confirmed"
    );
    assert_eq!(
        freshness(&mirror.recovery_day("2026-09-26").await),
        "partial"
    );
    let missing = mirror.recovery_day("2026-09-27").await;
    assert_eq!(freshness(&missing), "unconfirmed");
    assert_eq!(missing["derived_scores"]["strain"], absent("score_0_100"));

    // A late sync: the imported namespace arrives first, the computed one an hour later.
    mirror
        .push(
            SOURCE,
            IMPORTED,
            Window::Days("2026-09-27", "2026-09-28"),
            &[daily("2026-09-27", json!({"strain": 5.2}))],
            "2026-09-28T04:00:00.000Z",
        )
        .await;
    let half_synced = mirror.recovery_day("2026-09-27").await;
    assert_eq!(
        half_synced["derived_scores"]["strain"],
        measured(json!(5.2), "score_0_100", IMPORTED)
    );
    assert_eq!(
        half_synced["noop"]["last_push_at"],
        "2026-09-26T03:51:38+02:00"
    );
    assert_eq!(freshness(&half_synced), "unconfirmed");
    assert_eq!(half_synced["noop"]["coverage"]["daily_metrics"], "unknown");

    mirror
        .push(
            SOURCE,
            COMPUTED,
            Window::Days("2026-09-27", "2026-09-28"),
            &[daily(
                "2026-09-27",
                json!({"strain": 4.0, "recovery": 58.0}),
            )],
            "2026-09-28T05:00:00.000Z",
        )
        .await;
    let synced = mirror.recovery_day("2026-09-27").await;
    assert_eq!(synced["noop"]["last_push_at"], "2026-09-26T03:51:38+02:00");
    assert_eq!(freshness(&synced), "unconfirmed");
    assert_eq!(
        synced["derived_scores"],
        json!({
            "recovery": measured(json!(58.0), "score_0_100", COMPUTED),
            "strain": measured(json!(5.2), "score_0_100", IMPORTED),
        })
    );
    assert_eq!(
        synced["noop"]["coverage"],
        json!({"daily_metrics": "covered", "sleep_sessions": "unknown"})
    );

    // Daily rows alone do not confirm the Sleep Night. Both sleep namespaces must finish too.
    for device in [IMPORTED, COMPUTED] {
        mirror
            .push(
                SOURCE,
                device,
                Window::Starts(1_790_373_600, 1_790_546_400),
                &[],
                "2026-09-28T06:00:00.000Z",
            )
            .await;
    }
    let completed = mirror.recovery_day("2026-09-27").await;
    assert_eq!(
        completed["noop"]["last_push_at"],
        "2026-09-28T06:00:00+02:00"
    );
    assert_eq!(freshness(&completed), "confirmed");
}

#[tokio::test]
async fn staged_replacements_do_not_advance_recovery_freshness() {
    let mut mirror = real_mirror().await;
    for device in [IMPORTED, COMPUTED] {
        mirror
            .push_first_part(
                SOURCE,
                device,
                REAL_DAYS,
                &[daily("2026-09-26", json!({"recovery": 75.0}))],
                2,
                "2026-09-27T04:00:00.000Z",
            )
            .await;
    }

    let body = mirror.recovery_day("2026-09-26").await;
    assert_eq!(body["noop"]["freshness"], "partial");
    assert_eq!(body["noop"]["last_push_at"], "2026-09-26T03:51:38+02:00");
    assert_eq!(body["derived_scores"]["recovery"], absent("score_0_100"));
}

#[tokio::test]
async fn confirming_recovery_requires_completed_daily_and_sleep_windows() {
    let mut mirror = real_mirror().await;
    for device in [IMPORTED, COMPUTED] {
        mirror
            .push(SOURCE, device, REAL_DAYS, &[], "2026-09-27T04:00:00.000Z")
            .await;
    }
    let daily_only = mirror.recovery_day("2026-09-26").await;
    assert_eq!(daily_only["noop"]["freshness"], "partial");
    assert_eq!(
        daily_only["noop"]["last_push_at"],
        "2026-09-26T03:51:38+02:00"
    );

    for device in [IMPORTED, COMPUTED] {
        mirror
            .push(SOURCE, device, REAL_STARTS, &[], "2026-09-27T05:00:00.000Z")
            .await;
    }
    let completed = mirror.recovery_day("2026-09-26").await;
    assert_eq!(completed["noop"]["freshness"], "confirmed");
    assert_eq!(
        completed["noop"]["last_push_at"],
        "2026-09-27T06:00:00+02:00"
    );
}

#[tokio::test]
async fn historical_replacements_do_not_confirm_a_more_recent_day() {
    let mut mirror = real_mirror().await;
    for device in [IMPORTED, COMPUTED] {
        for window in [
            Window::Days("2026-09-19", "2026-09-21"),
            Window::Starts(SEP_20 - 86_400, SEP_20 + 86_400),
        ] {
            mirror
                .push(SOURCE, device, window, &[], "2026-09-27T04:00:00.000Z")
                .await;
        }
    }
    let body = mirror.recovery_day("2026-09-26").await;
    assert_eq!(body["noop"]["freshness"], "partial");
    assert_eq!(
        body["noop"]["coverage"],
        json!({
            "daily_metrics": "covered", "sleep_sessions": "covered"
        })
    );
}

#[tokio::test]
async fn freshness_is_unknown_until_both_namespaces_pushed() {
    let mut mirror = Mirror::empty().await;
    mirror
        .push(
            SOURCE,
            IMPORTED,
            REAL_DAYS,
            &real_imported_days(),
            REAL_PUSH_AT,
        )
        .await;
    let body = mirror.recovery_day("2026-09-14").await;

    assert_eq!(body["noop"]["installation_id"], SOURCE);
    assert_eq!(body["noop"]["last_push_at"], Value::Null);
    assert_eq!(body["noop"]["freshness"], "unknown");
    assert_eq!(body["noop"]["coverage"]["daily_metrics"], "unknown");
    assert_eq!(
        body["derived_scores"]["strain"],
        measured(json!(3.1), "score_0_100", IMPORTED)
    );
}

#[tokio::test]
async fn only_applied_windows_in_both_namespaces_establish_coverage() {
    let real = real_mirror().await;
    let coverage = |body: &Value| body["noop"]["coverage"].clone();

    // A covered night without sessions is a recorded absence.
    let no_night = real.recovery_day("2026-09-14").await;
    assert_eq!(no_night["sleep"]["sessions"], json!([]));
    assert_eq!(
        coverage(&no_night),
        json!({"daily_metrics": "covered", "sleep_sessions": "covered"})
    );
    // The night waking 09-13 may have started before the windows' first day.
    assert_eq!(
        coverage(&real.recovery_day("2026-09-13").await),
        json!({"daily_metrics": "covered", "sleep_sessions": "unknown"})
    );
    assert_eq!(
        coverage(&real.recovery_day("2026-09-27").await),
        json!({"daily_metrics": "unknown", "sleep_sessions": "unknown"})
    );

    let mut mirror = Mirror::empty().await;
    mirror
        .push(SOURCE, IMPORTED, REAL_DAYS, &[], REAL_PUSH_AT)
        .await;
    mirror
        .push(
            SOURCE,
            COMPUTED,
            Window::Days("2026-09-20", "2026-09-27"),
            &[],
            REAL_PUSH_AT,
        )
        .await;
    mirror
        .push(SOURCE, IMPORTED, REAL_STARTS, &[], REAL_PUSH_AT)
        .await;
    // Adjacent computed windows jointly cover 2026-09-20's night...
    mirror
        .push(
            SOURCE,
            COMPUTED,
            Window::Starts(1_789_250_400, SEP_20),
            &[],
            REAL_PUSH_AT,
        )
        .await;
    mirror
        .push(
            SOURCE,
            COMPUTED,
            Window::Starts(SEP_20, 1_790_460_000),
            &[],
            REAL_PUSH_AT,
        )
        .await;
    assert_eq!(
        coverage(&mirror.recovery_day("2026-09-20").await),
        json!({"daily_metrics": "covered", "sleep_sessions": "covered"})
    );
    // ...while a day only the imported namespace replaced stays unknown.
    assert_eq!(
        coverage(&mirror.recovery_day("2026-09-15").await)["daily_metrics"],
        "unknown"
    );

    // A staged, incomplete replacement establishes nothing.
    let mut staged = Mirror::empty().await;
    staged
        .push(SOURCE, IMPORTED, REAL_STARTS, &[], REAL_PUSH_AT)
        .await;
    staged
        .push_first_part(
            SOURCE,
            COMPUTED,
            REAL_STARTS,
            &[sleep(SEP_20 - 3_600, SEP_20 + 3_600, json!({}))],
            2,
            REAL_PUSH_AT,
        )
        .await;
    assert_eq!(
        coverage(&staged.recovery_day("2026-09-20").await)["sleep_sessions"],
        "unknown"
    );
}

#[tokio::test]
async fn reads_only_the_active_installation_and_strap_namespaces() {
    let mut mirror = Mirror::empty().await;
    // An older installation of NOOP reported the same strap before the current one took over.
    for device in [IMPORTED, COMPUTED] {
        mirror
            .push(
                OTHER_SOURCE,
                device,
                REAL_DAYS,
                &[daily("2026-09-14", json!({"recovery": 99.0}))],
                "2026-09-20T00:00:00.000Z",
            )
            .await;
    }
    for device in [IMPORTED, COMPUTED] {
        mirror
            .push(SOURCE, device, REAL_DAYS, &[], REAL_PUSH_AT)
            .await;
    }
    // A namespace that is not the strap's is never read, however recent.
    mirror
        .push(
            SOURCE,
            "other-strap",
            REAL_DAYS,
            &[daily("2026-09-14", json!({"recovery": 77.0}))],
            "2026-09-27T00:00:00.000Z",
        )
        .await;
    let body = mirror.recovery_day("2026-09-14").await;

    assert_eq!(body["noop"]["installation_id"], SOURCE);
    assert_eq!(body["noop"]["last_push_at"], Value::Null);
    assert_eq!(body["noop"]["freshness"], "unknown");
    assert_eq!(body["derived_scores"]["recovery"], absent("score_0_100"));
    assert_eq!(body["noop"]["coverage"]["daily_metrics"], "covered");
}

#[tokio::test]
async fn rejects_a_sleep_night_over_the_session_cap_instead_of_truncating() {
    // Short sessions every ten minutes from 00:00 on 2026-09-20.
    let naps = |count: i64| -> Vec<Value> {
        (0..count)
            .map(|index| {
                let start = SEP_20 + index * 600;
                sleep(start, start + 300, json!({}))
            })
            .collect()
    };

    let full = covered_mirror(&[], &[], &[], &naps(24)).await;
    let body = full.recovery_day("2026-09-20").await;
    assert_eq!(body["sleep"]["sessions"].as_array().unwrap().len(), 24);

    let over = covered_mirror(&[], &[], &[], &naps(25)).await;
    let (status, body) = over.recovery("2026-09-20", Some(API_KEY)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        body,
        json!({
            "error": "The Sleep Night waking on 2026-09-20 has more than 24 sleep sessions; \
                      refusing to return a truncated night",
            "status": 422,
        })
    );
    // The previous night is unaffected.
    let previous = over.recovery_day("2026-09-19").await;
    assert_eq!(previous["sleep"]["sessions"], json!([]));
}
