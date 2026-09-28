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
}

/// A NOOP push mirror that fixtures are pushed into through the push endpoint.
pub struct Mirror {
    pub db: SqlitePool,
    ids: u64,
}

impl Mirror {
    pub async fn empty() -> Self {
        Self {
            db: super::memory_noop_db().await,
            ids: 0,
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
}
