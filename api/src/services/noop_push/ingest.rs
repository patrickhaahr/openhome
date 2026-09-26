//! Batch ingest for NOOP push (`.noop/PUSH_PROTOCOL.md`, "NDJSON request", "Append delivery and
//! cursors", "Authoritative rolling-window delivery", "Acceptance, errors, and retry
//! idempotency").
//!
//! Pipeline: decode the content coding within the 4 MiB decoded bound, frame the NDJSON (header
//! line plus exactly `recordCount` record lines), validate the header and every record against
//! the v1 registry, then, in one write transaction, consult the batch ledger and reject a
//! conflicting `batchId`. An append batch then either replays the stored ack or upserts its
//! records; a replace-window part goes through [`replace_window::accept`](super::replace_window),
//! which stages it and applies the replacement once every part is present.

use std::collections::HashSet;
use std::io::Read;

use chrono::NaiveDate;
use flate2::read::MultiGzDecoder;
use serde::de::{self, IgnoredAny, MapAccess, Visitor};
use serde::{Deserialize, Deserializer};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use sqlx::{SqliteConnection, SqlitePool};
use thiserror::Error;
use uuid::Uuid;

use super::SUPPORTED_VERSIONS;
use super::registry::{self, Column, ColumnType, Selector, Stream};
use super::replace_window::{self, PartOutcome};

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
    pub fn code(&self) -> &'static str {
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

/// Content coding of the request entity. Batch identity is the decoded bytes, never the coding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContentCoding {
    Identity,
    Gzip,
}

impl ContentCoding {
    /// Parses a single `Content-Encoding` value (absent means identity). Returns `None` for any
    /// other coding, including stacked codings.
    pub fn parse(value: Option<&str>) -> Option<Self> {
        let value = value.map(|value| value.trim().to_ascii_lowercase());
        match value.as_deref() {
            None | Some("identity") => Some(Self::Identity),
            Some("gzip" | "x-gzip") => Some(Self::Gzip),
            _ => None,
        }
    }
}

/// Removes the content coding, enforcing the decoded bound while decompressing so a small gzip
/// entity cannot expand without limit.
pub fn decode_entity(coding: ContentCoding, wire: Vec<u8>) -> Result<Vec<u8>, IngestError> {
    let decoded = match coding {
        ContentCoding::Identity => wire,
        ContentCoding::Gzip => {
            let mut decoded = Vec::new();
            MultiGzDecoder::new(wire.as_slice())
                .take(MAX_DECODED_BYTES as u64 + 1)
                .read_to_end(&mut decoded)
                .map_err(|_| IngestError::Malformed("invalid_content_encoding"))?;
            decoded
        }
    };
    if decoded.len() > MAX_DECODED_BYTES {
        return Err(IngestError::TooLarge);
    }
    Ok(decoded)
}

/// Result of a successfully accepted (or replayed) batch.
#[derive(Debug)]
pub struct Accepted {
    /// The acknowledgement JSON, byte-identical on every replay.
    pub ack: String,
    pub stream: &'static str,
    pub records: usize,
    /// True when the ack came from the ledger and no rows were written.
    pub replayed: bool,
}

/// A decoded, validated batch, its decoded entity (staged for replace-window parts) and the
/// entity's SHA-256, ready to apply.
#[derive(Debug)]
pub struct PreparedBatch {
    batch: Batch,
    entity: Vec<u8>,
    body_sha256: Vec<u8>,
}

/// The CPU-bound half of ingest (decode up to 4 MiB, frame, validate, hash). Synchronous so the
/// caller can run it off the async workers.
pub fn prepare(coding: ContentCoding, wire: Vec<u8>) -> Result<PreparedBatch, IngestError> {
    let entity = decode_entity(coding, wire)?;
    let batch = parse_batch(&entity)?;
    let body_sha256 = Sha256::digest(&entity).to_vec();
    Ok(PreparedBatch {
        batch,
        entity,
        body_sha256,
    })
}

/// Applies a prepared batch idempotently. A `batchId` already in the ledger with different bytes
/// is a conflict. An append batch replays the ledger ack for a byte-identical retry or upserts
/// its records; a replace-window part is staged and the replacement applied once complete.
pub async fn apply(pool: &SqlitePool, prepared: PreparedBatch) -> Result<Accepted, IngestError> {
    let PreparedBatch {
        batch,
        entity,
        body_sha256,
    } = prepared;

    // IMMEDIATE takes the write lock up front, so concurrent retries of one batch serialize on
    // the ledger instead of both applying it.
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    // Rotation clears the ledger and replacement metadata in the same transaction as the ID
    // update (receiver_state_rotation trigger), starting a fresh idempotency generation.
    let receiver_state_id: String =
        sqlx::query_scalar("SELECT receiver_state_id FROM receiver_state WHERE id = 1")
            .fetch_one(&mut *tx)
            .await?;

    let prior: Option<(Vec<u8>, String)> = sqlx::query_as(
        "SELECT body_sha256, ack FROM batch_ledger \
         WHERE receiver_state_id = ? AND source_id = ? AND device_id = ? AND batch_id = ?",
    )
    .bind(&receiver_state_id)
    .bind(&batch.source_id)
    .bind(&batch.device_id)
    .bind(&batch.batch_id)
    .fetch_optional(&mut *tx)
    .await?;
    // Dropping the transaction on an early return rolls it back; nothing was written.
    let stored_ack = match prior {
        Some((stored_sha256, _)) if stored_sha256 != body_sha256.as_slice() => {
            return Err(IngestError::Conflict("batch_conflict"));
        }
        Some((_, ack)) => Some(ack),
        None => None,
    };
    let replay = |ack: String| Accepted {
        ack,
        stream: batch.stream.name,
        records: batch.records.len(),
        replayed: true,
    };

    match &batch.delivery {
        BatchDelivery::Append { .. } => {
            if let Some(ack) = stored_ack {
                return Ok(replay(ack));
            }
            upsert(
                &mut tx,
                batch.stream,
                &batch.source_id,
                &batch.device_id,
                &batch.records,
            )
            .await?;
        }
        BatchDelivery::ReplaceWindow(window) => {
            let outcome = replace_window::accept(
                &mut tx,
                &receiver_state_id,
                &batch,
                window,
                &entity,
                stored_ack.is_some(),
            )
            .await?;
            if let (PartOutcome::Retry, Some(ack)) = (outcome, &stored_ack) {
                return Ok(replay(ack.clone()));
            }
        }
    }

    let ack = batch.ack();
    sqlx::query(
        "INSERT INTO batch_ledger \
             (receiver_state_id, source_id, device_id, batch_id, stream, body_sha256, ack) \
             VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&receiver_state_id)
    .bind(&batch.source_id)
    .bind(&batch.device_id)
    .bind(&batch.batch_id)
    .bind(batch.stream.name)
    .bind(body_sha256.as_slice())
    .bind(&ack)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    Ok(Accepted {
        ack,
        stream: batch.stream.name,
        records: batch.records.len(),
        replayed: false,
    })
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
    pub(super) fn selector(&self) -> Selector {
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

/// Frames and validates a decoded NDJSON entity as a batch of either delivery mode.
pub fn parse_batch(entity: &[u8]) -> Result<Batch, IngestError> {
    // Every line, including the last, ends with LF; there is no trailing material.
    let content = entity.strip_suffix(b"\n").ok_or(MALFORMED_NDJSON)?;
    let mut lines = content.split(|byte| *byte == b'\n');
    let header_line = object_line(lines.next().unwrap_or_default())?;
    let header = parse_header(header_line)?;
    let window = match &header.delivery {
        BatchDelivery::Append { .. } => None,
        BatchDelivery::ReplaceWindow(window) => Some(window),
    };

    let expected = header.record_count as usize;
    let mut records = Vec::with_capacity(expected);
    let mut keys = HashSet::with_capacity(expected);
    for line in lines {
        let line = object_line(line)?;
        if records.len() == expected {
            return Err(RECORD_COUNT_MISMATCH);
        }
        let record = parse_record(header.stream, line)?;
        if let Some(window) = window {
            // The selector is the first key column (registry invariant). Day keys must be real
            // `YYYY-MM-DD` dates; a record outside the declared window would escape its
            // absence-means-delete scope.
            let selector_value = &record.key[0];
            if matches!(selector_value, KeyValue::Text(day) if !is_day(day)) {
                return Err(INVALID_RECORD);
            }
            if !window.bounds.contains(selector_value) {
                return Err(IngestError::Unprocessable("record_outside_window"));
            }
        }
        if !keys.insert(record.key.clone()) {
            return Err(DUPLICATE_KEY);
        }
        records.push(record);
    }
    if records.len() != expected {
        return Err(RECORD_COUNT_MISMATCH);
    }

    Ok(Batch {
        protocol_version: header.protocol_version,
        batch_id: header.batch_id,
        source_id: header.source_id,
        device_id: header.device_id,
        stream: header.stream,
        delivery: header.delivery,
        records,
    })
}

/// A line must be exactly one JSON object: no blank line, BOM, array, or surrounding whitespace.
fn object_line(line: &[u8]) -> Result<&[u8], IngestError> {
    if line.first() == Some(&b'{') && line.last() == Some(&b'}') {
        Ok(line)
    } else {
        Err(MALFORMED_NDJSON)
    }
}

/// Deserializes one line. A JSON syntax error is malformed NDJSON (400); valid JSON of the wrong
/// shape is `invalid` (422).
fn parse_line<'a, T: Deserialize<'a>>(
    line: &'a [u8],
    invalid: IngestError,
) -> Result<T, IngestError> {
    serde_json::from_slice(line).map_err(|_| {
        if serde_json::from_slice::<IgnoredAny>(line).is_ok() {
            invalid
        } else {
            MALFORMED_NDJSON
        }
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct VersionProbe {
    protocol_version: Option<Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawHeader {
    #[serde(rename = "type")]
    kind: String,
    protocol_version: String,
    batch_id: String,
    source_id: String,
    device_id: String,
    stream: String,
    delivery: String,
    record_count: u32,
    #[serde(deserialize_with = "required_nullable")]
    start_cursor: Option<Cursor>,
    #[serde(deserialize_with = "required_nullable")]
    end_cursor: Option<Cursor>,
    #[serde(default, deserialize_with = "presence")]
    window: Option<Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawWindow {
    replacement_id: String,
    selector: String,
    start_inclusive: Value,
    end_exclusive: Value,
    part: u32,
    parts: u32,
}

/// A member that must be present but may be `null` (plain `Option` members may be omitted).
fn required_nullable<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::deserialize(deserializer)
}

/// Records that a member was present with any value, including `null`.
fn presence<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<Value>, D::Error> {
    Value::deserialize(deserializer).map(Some)
}

struct Header {
    protocol_version: String,
    batch_id: String,
    source_id: String,
    device_id: String,
    stream: &'static Stream,
    record_count: u32,
    delivery: BatchDelivery,
}

fn parse_header(line: &[u8]) -> Result<Header, IngestError> {
    // Check the version first: a batch from another major version may not share this shape.
    let probe: VersionProbe = parse_line(line, INVALID_HEADER)?;
    let supported = matches!(
        &probe.protocol_version,
        Some(Value::String(version)) if SUPPORTED_VERSIONS.contains(&version.as_str())
    );
    if !supported {
        return Err(IngestError::Unprocessable("unsupported_protocol_version"));
    }

    let header: RawHeader = parse_line(line, INVALID_HEADER)?;
    if header.kind != "batch" {
        return Err(INVALID_HEADER);
    }
    let stream =
        registry::stream(&header.stream).ok_or(IngestError::Unprocessable("unsupported_stream"))?;
    // The registry fixes each stream's delivery mode; `window` belongs to replace_window only.
    let selector = match (stream.replace_window(), header.delivery.as_str()) {
        (None, "append") if header.window.is_none() => None,
        (Some(spec), "replace_window") => Some(spec.selector),
        _ => return Err(INVALID_HEADER),
    };
    if !is_canonical_uuid(&header.batch_id)
        || !is_canonical_uuid(&header.source_id)
        || header.device_id.is_empty()
    {
        return Err(INVALID_HEADER);
    }
    if header.record_count > MAX_RECORDS {
        return Err(INVALID_HEADER);
    }

    let delivery = match selector {
        None => {
            if header.record_count == 0 {
                return Err(EMPTY_BATCH);
            }
            let end_cursor = header.end_cursor.ok_or(INVALID_CURSOR)?;
            if !cursors_are_valid(
                header.start_cursor.as_ref(),
                &end_cursor,
                header.record_count,
            ) {
                return Err(INVALID_CURSOR);
            }
            BatchDelivery::Append { end_cursor }
        }
        Some(selector) => {
            if header.start_cursor.is_some() || header.end_cursor.is_some() {
                return Err(INVALID_CURSOR);
            }
            let window = parse_window(selector, header.window)?;
            // An empty window is one zero-record part; a multi-part replacement never has an
            // empty part.
            if header.record_count == 0 && window.parts != 1 {
                return Err(EMPTY_BATCH);
            }
            BatchDelivery::ReplaceWindow(window)
        }
    };

    Ok(Header {
        protocol_version: header.protocol_version,
        batch_id: header.batch_id,
        source_id: header.source_id,
        device_id: header.device_id,
        stream,
        record_count: header.record_count,
        delivery,
    })
}

/// Validates the `window` member: canonical `replacementId`, the stream's selector, non-empty
/// half-open bounds of the selector's type, and `1 <= part <= parts`.
fn parse_window(selector: Selector, window: Option<Value>) -> Result<Window, IngestError> {
    let raw: RawWindow = window
        .and_then(|window| serde_json::from_value(window).ok())
        .ok_or(INVALID_WINDOW)?;
    if !is_canonical_uuid(&raw.replacement_id) || raw.selector != selector.wire_name() {
        return Err(INVALID_WINDOW);
    }
    if !(1..=raw.parts).contains(&raw.part) {
        return Err(INVALID_WINDOW);
    }
    let bounds = match (selector, raw.start_inclusive, raw.end_exclusive) {
        (Selector::Day, Value::String(start), Value::String(end))
            if is_day(&start) && is_day(&end) && start < end =>
        {
            Bounds::Day { start, end }
        }
        (Selector::StartTs, Value::Number(start), Value::Number(end)) => {
            match (start.as_i64(), end.as_i64()) {
                (Some(start), Some(end)) if start < end => Bounds::StartTs { start, end },
                _ => return Err(INVALID_WINDOW),
            }
        }
        _ => return Err(INVALID_WINDOW),
    };
    Ok(Window {
        replacement_id: raw.replacement_id,
        bounds,
        part: raw.part,
        parts: raw.parts,
    })
}

/// A canonical `YYYY-MM-DD` calendar date.
fn is_day(value: &str) -> bool {
    value.len() == 10
        && NaiveDate::parse_from_str(value, "%Y-%m-%d")
            .is_ok_and(|date| date.format("%Y-%m-%d").to_string() == value)
}

fn is_canonical_uuid(value: &str) -> bool {
    Uuid::parse_str(value).is_ok_and(|uuid| uuid.hyphenated().to_string() == value)
}

/// Enforces `startCursor.rowId < first < ... <= final == endCursor.rowId` as far as the receiver
/// can see it: records carry no row IDs, so `recordCount` strictly increasing positive positions
/// must fit in `(startCursor.rowId, endCursor.rowId]` (`(0, endCursor.rowId]` for a first batch).
/// `keySha256` is checked for shape only and is never recomputed or interpreted.
///
/// Requiring positive row IDs, and using 0 as the floor when `startCursor` is null, is
/// deliberate: it matches SQLite `rowid` semantics (the protocol maps `rowId` to it) and the
/// sender's own invariant (`.noop/PushModels.kt`, `PushAppendRecord`: `rowId > 0`).
fn cursors_are_valid(start: Option<&Cursor>, end: &Cursor, record_count: u32) -> bool {
    let well_formed = |cursor: &Cursor| {
        cursor.row_id > 0
            && cursor.key_sha256.len() == 64
            && cursor
                .key_sha256
                .bytes()
                .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    };
    if !well_formed(end) || !start.is_none_or(well_formed) {
        return false;
    }
    let floor = start.map_or(0, |cursor| cursor.row_id);
    // Both row IDs are positive, so the difference cannot overflow.
    end.row_id - floor >= i64::from(record_count)
}

#[derive(Deserialize)]
struct RawRecord {
    #[serde(rename = "type")]
    kind: String,
    key: Members,
    data: Members,
}

/// A JSON object that rejects duplicate member names instead of keeping the last one.
struct Members(Map<String, Value>);

impl<'de> Deserialize<'de> for Members {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct MembersVisitor;

        impl<'de> Visitor<'de> for MembersVisitor {
            type Value = Members;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a JSON object")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut access: A) -> Result<Members, A::Error> {
                let mut members = Map::new();
                while let Some(name) = access.next_key::<String>()? {
                    let value = access.next_value()?;
                    if members.insert(name, value).is_some() {
                        return Err(de::Error::custom("duplicate object member"));
                    }
                }
                Ok(Members(members))
            }
        }

        deserializer.deserialize_map(MembersVisitor)
    }
}

fn parse_record(stream: &Stream, line: &[u8]) -> Result<Record, IngestError> {
    let raw: RawRecord = parse_line(line, INVALID_RECORD)?;
    if raw.kind != "record" {
        return Err(INVALID_RECORD);
    }
    // Keys contain exactly the registry columns; unknown data members are ignored.
    if raw.key.0.len() != stream.key.len() {
        return Err(INVALID_RECORD);
    }
    let key = stream
        .key
        .iter()
        .map(|column| {
            raw.key
                .0
                .get(column.name)
                .and_then(|value| key_value(column, value))
        })
        .collect::<Option<Vec<_>>>()
        .ok_or(INVALID_RECORD)?;
    let data = stream
        .data
        .iter()
        .map(|column| {
            raw.data
                .0
                .get(column.name)
                .and_then(|value| data_value(column, value))
        })
        .collect::<Option<Vec<_>>>()
        .ok_or(INVALID_RECORD)?;
    Ok(Record { key, data })
}

fn key_value(column: &Column, value: &Value) -> Option<KeyValue> {
    match (column.ty, value) {
        (ColumnType::Integer, Value::Number(number)) => number.as_i64().map(KeyValue::Integer),
        (ColumnType::Text, Value::String(text)) => Some(KeyValue::Text(text.clone())),
        _ => None,
    }
}

/// Converts a data member to its registry type. Integers reject fractions and exponents; reals
/// accept any JSON number (always finite once parsed).
fn data_value(column: &Column, value: &Value) -> Option<DataValue> {
    match (column.ty, value) {
        (_, Value::Null) if column.nullable => Some(DataValue::Null),
        (ColumnType::Integer, Value::Number(number)) => number.as_i64().map(DataValue::Integer),
        (ColumnType::Real, Value::Number(number)) => number
            .as_f64()
            .filter(|real| real.is_finite())
            .map(DataValue::Real),
        (ColumnType::Text, Value::String(text)) => Some(DataValue::Text(text.clone())),
        (ColumnType::Boolean, Value::Bool(flag)) => Some(DataValue::Boolean(*flag)),
        _ => None,
    }
}
