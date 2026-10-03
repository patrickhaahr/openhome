//! A NOOP push mirror filled through the real push endpoint, for training-read tests.
#![allow(dead_code)]

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use openhome_api::routes::noop_push::{PUSH_PATH, PushState, router as push_router};
use openhome_api::services::noop_push::PushToken;
use serde_json::{Value, json};
use sqlx::SqlitePool;
use tower::ServiceExt;

const PUSH_TOKEN: &str = "test-push-token";

#[derive(Clone, Copy)]
pub enum Window {
    /// A `dailyMetric` window of `YYYY-MM-DD` days.
    Days(&'static str, &'static str),
    /// A `sleepSession` window of Unix seconds.
    Starts(i64, i64),
    /// A `workout` window of Unix seconds.
    WorkoutStarts(i64, i64),
    /// A `journal` window of `YYYY-MM-DD` days.
    JournalDays(&'static str, &'static str),
}

/// Records one append batch may carry under the push protocol.
const MAX_BATCH_RECORDS: usize = 5_000;

/// A `workout` record from `start_ts` to `end_ts`: every metric null except `fields`.
pub fn noop_workout(start_ts: i64, end_ts: i64, sport: &str, source: &str, fields: Value) -> Value {
    let mut data = json!({
        "endTs": end_ts, "source": source, "durationS": null, "energyKcal": null, "avgHr": null,
        "maxHr": null, "strain": null, "distanceM": null, "zonesJSON": null, "notes": null,
        "routePolyline": null, "steps": null,
    });
    let Value::Object(fields) = fields else {
        panic!("fields must be a JSON object");
    };
    for (name, value) in fields {
        assert!(data.get(&name).is_some(), "unknown field {name}");
        data[&name] = value;
    }
    json!({"type": "record", "key": {"startTs": start_ts, "sport": sport}, "data": data})
}

/// A NOOP push mirror that fixtures are pushed into through the push endpoint.
pub struct Mirror {
    pub db: SqlitePool,
    ids: u64,
    /// The sender's last append row id, shared by every append stream.
    row_id: i64,
}

impl Mirror {
    pub async fn empty() -> Self {
        Self {
            db: super::memory_noop_db().await,
            ids: 0,
            row_id: 0,
        }
    }

    fn next_id(&mut self) -> String {
        self.ids += 1;
        format!("00000000-0000-4000-8000-{:012x}", self.ids)
    }

    /// Pushes one complete replacement window, recorded as accepted at `accepted_at`.
    pub async fn push(
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
    pub async fn push_first_part(
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
            Window::WorkoutStarts(start, end) => ("workout", "startTs", json!(start), json!(end)),
            Window::JournalDays(start, end) => ("journal", "day", json!(start), json!(end)),
        };
        let header = json!({
            "type": "batch",
            "protocolVersion": "1.0",
            "batchId": self.next_id(),
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
        self.send(&header, records, accepted_at).await;
    }

    /// Appends `(ts, bpm)` strap heart rate samples as `hrSample` batches of at most 5,000
    /// records.
    pub async fn push_hr_samples(
        &mut self,
        source: &str,
        device: &str,
        samples: &[(i64, i64)],
        accepted_at: &str,
    ) {
        for chunk in samples.chunks(MAX_BATCH_RECORDS) {
            let records: Vec<Value> = chunk
                .iter()
                .map(
                    |&(ts, bpm)| json!({"type": "record", "key": {"ts": ts}, "data": {"bpm": bpm}}),
                )
                .collect();
            let cursor = |row_id: i64| json!({"rowId": row_id, "keySha256": "0".repeat(64)});
            let start_cursor = (self.row_id > 0).then(|| cursor(self.row_id));
            self.row_id += records.len() as i64;
            let header = json!({
                "type": "batch",
                "protocolVersion": "1.0",
                "batchId": self.next_id(),
                "sourceId": source,
                "deviceId": device,
                "stream": "hrSample",
                "delivery": "append",
                "recordCount": records.len(),
                "startCursor": start_cursor,
                "endCursor": cursor(self.row_id),
            });
            self.send(&header, &records, accepted_at).await;
        }
    }

    /// Posts one batch through the push endpoint and records it as accepted at `accepted_at`.
    async fn send(&self, header: &Value, records: &[Value], accepted_at: &str) {
        let mut entity = String::new();
        for line in std::iter::once(header).chain(records) {
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
            .bind(header["batchId"].as_str().unwrap())
            .execute(&self.db)
            .await
            .unwrap();
    }
}
