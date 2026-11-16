//! Batch ingest for NOOP push (`docs/noop/PUSH_PROTOCOL_DELIVERY.md`, "NDJSON request", "Append
//! delivery and cursors", "Authoritative rolling-window delivery";
//! `docs/noop/PUSH_PROTOCOL_STREAMS_AND_ERRORS.md`, "Acceptance, errors, and retry idempotency").
//!
//! Pipeline: decode the content coding within the 4 MiB decoded bound, frame the NDJSON (header
//! line plus exactly `recordCount` record lines), validate the header and every record against
//! the v1 registry, then, in one write transaction, consult the batch ledger and reject a
//! conflicting `batchId`. An append batch then either replays the stored ack or upserts its
//! records; a replace-window part goes through [`replace_window::accept`](super::replace_window),
//! which stages it and applies the replacement once every part is present.

use serde::Deserialize;
use serde_json::Value;
use sqlx::SqliteConnection;
use thiserror::Error;

use super::registry::{Selector, Stream};

mod apply;
mod parse;

pub use apply::{Accepted, ContentCoding, PreparedBatch, apply, decode_entity, prepare};
pub use parse::parse_batch;

/// Maximum record lines in one batch.
pub const MAX_RECORDS: u32 = 5_000;
/// Maximum decoded UTF-8 NDJSON entity, including newlines.
pub const MAX_DECODED_BYTES: usize = 4 * 1024 * 1024;
/// Maximum encoded wire entity: the decoded bound plus the sender's allowance for gzip framing.
pub const MAX_WIRE_BYTES: usize = MAX_DECODED_BYTES + 64 * 1024;

/// A rejected batch. Every variant maps to one bounded, machine-readable error code.
#[derive(Debug, Error)]
pub enum IngestError {
    /// Framing or JSON syntax is broken (HTTP 400).
    #[error("malformed request entity ({0})")]
    Malformed(&'static str),
    /// Encoded or decoded entity exceeds its bound (HTTP 413).
    #[error("request entity exceeds the size limit")]
    TooLarge,
    /// Well-formed JSON that violates the protocol or registry (HTTP 422).
    #[error("unprocessable batch ({0})")]
    Unprocessable(&'static str),
    /// Conflicting reuse of a `batchId`, `replacementId` or part number, or a late part of a
    /// superseded replacement generation (HTTP 409).
    #[error("conflicting batch ({0})")]
    Conflict(&'static str),
    #[error(transparent)]
    Storage(#[from] sqlx::Error),
    /// Receiver-side invariant violation, e.g. a staged part that no longer parses (HTTP 500).
    #[error("internal ingest failure ({0})")]
    Internal(&'static str),
}

impl IngestError {
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Malformed(code) | Self::Unprocessable(code) | Self::Conflict(code) => code,
            Self::TooLarge => "payload_too_large",
            Self::Storage(_) => "storage_unavailable",
            Self::Internal(_) => "internal_error",
        }
    }
}

const MALFORMED_NDJSON: IngestError = IngestError::Malformed("malformed_ndjson");
const RECORD_COUNT_MISMATCH: IngestError = IngestError::Malformed("record_count_mismatch");
const INVALID_HEADER: IngestError = IngestError::Unprocessable("invalid_header");
const INVALID_CURSOR: IngestError = IngestError::Unprocessable("invalid_cursor");
const INVALID_RECORD: IngestError = IngestError::Unprocessable("invalid_record");
const INVALID_WINDOW: IngestError = IngestError::Unprocessable("invalid_window");
pub(super) const DUPLICATE_KEY: IngestError = IngestError::Unprocessable("duplicate_key");
const EMPTY_BATCH: IngestError = IngestError::Unprocessable("empty_batch");

/// What the batch ledger says about a batch's `(receiver_state_id, source_id, device_id,
/// batch_id)` identity, read inside the write transaction that may accept it. The ledger is
/// scoped to the receiver generation, so after a rotation every batch is `Unseen` again.
///
/// A third outcome, a stored `batchId` whose decoded bytes differ, is a conflict and never
/// becomes a value: [`BatchHistory::read`] rejects it with `batch_conflict` (HTTP 409) before any
/// row changes.
#[derive(Debug)]
pub(super) enum BatchHistory {
    /// No acknowledgement stored: apply the batch (a replace-window part is staged).
    Unseen,
    /// Byte-identical retry of an acknowledged batch: replay `ack` unchanged.
    Identical { ack: String },
}

impl BatchHistory {
    async fn read(
        conn: &mut SqliteConnection,
        receiver_state_id: &str,
        batch: &Batch,
        body_sha256: &[u8],
    ) -> Result<Self, IngestError> {
        let prior: Option<(Vec<u8>, String)> = sqlx::query_as(
            "SELECT body_sha256, ack FROM batch_ledger \
             WHERE receiver_state_id = ? AND source_id = ? AND device_id = ? AND batch_id = ?",
        )
        .bind(receiver_state_id)
        .bind(&batch.source_id)
        .bind(&batch.device_id)
        .bind(&batch.batch_id)
        .fetch_optional(&mut *conn)
        .await?;
        match prior {
            None => Ok(Self::Unseen),
            Some((stored_sha256, ack)) if stored_sha256 == body_sha256 => {
                Ok(Self::Identical { ack })
            }
            Some(_) => Err(IngestError::Conflict("batch_conflict")),
        }
    }
}

/// Upserts records into the stream's typed table by `(source_id, device_id, natural key)`.
pub(super) async fn upsert(
    conn: &mut SqliteConnection,
    stream: &Stream,
    source_id: &str,
    device_id: &str,
    records: &[Record],
) -> Result<(), sqlx::Error> {
    for record in records {
        let mut query = sqlx::query(stream.upsert_sql)
            .bind(source_id)
            .bind(device_id);
        for value in &record.key {
            query = value.bind(query);
        }
        for value in &record.data {
            query = match value {
                DataValue::Null => query.bind(None::<i64>),
                DataValue::Integer(value) => query.bind(*value),
                DataValue::Real(value) => query.bind(*value),
                DataValue::Text(value) => query.bind(value.as_str()),
                DataValue::Boolean(value) => query.bind(*value),
            };
        }
        query.execute(&mut *conn).await?;
    }
    Ok(())
}

/// A fully validated batch of either delivery mode.
#[derive(Debug)]
pub struct Batch {
    protocol_version: String,
    pub(super) batch_id: String,
    pub(super) source_id: String,
    pub(super) device_id: String,
    pub(super) stream: &'static Stream,
    delivery: BatchDelivery,
    pub(super) records: Vec<Record>,
}

#[derive(Debug)]
enum BatchDelivery {
    Append { end_cursor: Cursor },
    ReplaceWindow(Window),
}

impl Batch {
    /// The acknowledgement: every member echoes the request, `keySha256` verbatim, and
    /// `endCursor` is `null` for a replace-window part.
    fn ack(&self) -> String {
        let end_cursor = match &self.delivery {
            BatchDelivery::Append { end_cursor } => serde_json::json!({
                "rowId": end_cursor.row_id,
                "keySha256": end_cursor.key_sha256,
            }),
            BatchDelivery::ReplaceWindow(_) => Value::Null,
        };
        serde_json::json!({
            "protocolVersion": self.protocol_version,
            "batchId": self.batch_id,
            "stream": self.stream.name,
            "deviceId": self.device_id,
            "endCursor": end_cursor,
            "acceptedRows": self.records.len(),
            "status": "accepted",
        })
        .to_string()
    }
}

/// The `window` header member of a replace-window part.
#[derive(Debug)]
pub(super) struct Window {
    pub(super) replacement_id: String,
    pub(super) bounds: Bounds,
    pub(super) part: u32,
    pub(super) parts: u32,
}

/// Half-open window bounds: `startInclusive <= selector < endExclusive`.
#[derive(Debug)]
pub(super) enum Bounds {
    /// Canonical `YYYY-MM-DD` days; they compare correctly as strings.
    Day { start: String, end: String },
    /// Unix seconds.
    StartTs { start: i64, end: i64 },
}

impl Bounds {
    pub(super) const fn selector(&self) -> Selector {
        match self {
            Self::Day { .. } => Selector::Day,
            Self::StartTs { .. } => Selector::StartTs,
        }
    }

    /// Canonical text of the bounds, as stored with the replacement to detect conflicting reuse.
    pub(super) fn canonical(&self) -> (String, String) {
        match self {
            Self::Day { start, end } => (start.clone(), end.clone()),
            Self::StartTs { start, end } => (start.to_string(), end.to_string()),
        }
    }

    /// Binds `startInclusive` then `endExclusive`.
    pub(super) fn bind<'q>(&'q self, query: SqliteQuery<'q>) -> SqliteQuery<'q> {
        match self {
            Self::Day { start, end } => query.bind(start.as_str()).bind(end.as_str()),
            Self::StartTs { start, end } => query.bind(*start).bind(*end),
        }
    }

    /// Whether a record's selector value (its first key column) lies inside the window.
    fn contains(&self, selector_value: &KeyValue) -> bool {
        match (self, selector_value) {
            (Self::Day { start, end }, KeyValue::Text(day)) => {
                start.as_str() <= day.as_str() && day.as_str() < end.as_str()
            }
            (Self::StartTs { start, end }, KeyValue::Integer(ts)) => start <= ts && ts < end,
            _ => false,
        }
    }
}

pub(super) type SqliteQuery<'q> =
    sqlx::query::Query<'q, sqlx::Sqlite, sqlx::sqlite::SqliteArguments<'q>>;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Cursor {
    row_id: i64,
    key_sha256: String,
}

#[derive(Debug)]
pub(super) struct Record {
    pub(super) key: Vec<KeyValue>,
    data: Vec<DataValue>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) enum KeyValue {
    Integer(i64),
    Text(String),
}

impl KeyValue {
    pub(super) fn bind<'q>(&'q self, query: SqliteQuery<'q>) -> SqliteQuery<'q> {
        match self {
            Self::Integer(value) => query.bind(*value),
            Self::Text(value) => query.bind(value.as_str()),
        }
    }
}

#[derive(Debug, PartialEq)]
enum DataValue {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Boolean(bool),
}
