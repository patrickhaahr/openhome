//! Append-batch ingest for NOOP push (`.noop/PUSH_PROTOCOL.md`, "NDJSON request", "Append
//! delivery and cursors", "Acceptance, errors, and retry idempotency").
//!
//! Pipeline: decode the content coding within the 4 MiB decoded bound, frame the NDJSON (header
//! line plus exactly `recordCount` record lines), validate the header and every record against
//! the v1 registry, then, in one write transaction, consult the batch ledger and either replay
//! the stored ack, reject a conflicting `batchId`, or upsert the records and record the ack.

use std::collections::HashSet;
use std::io::Read;

use flate2::read::MultiGzDecoder;
use serde::de::{self, IgnoredAny, MapAccess, Visitor};
use serde::{Deserialize, Deserializer};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use thiserror::Error;
use uuid::Uuid;

use super::SUPPORTED_VERSIONS;
use super::registry::{self, AppendStream, Column, ColumnType};

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
    /// `batchId` was already accepted with different decoded bytes (HTTP 409).
    #[error("batchId reused with different decoded bytes")]
    Conflict,
    #[error(transparent)]
    Storage(#[from] sqlx::Error),
}

impl IngestError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Malformed(code) | Self::Unprocessable(code) => code,
            Self::TooLarge => "payload_too_large",
            Self::Conflict => "batch_conflict",
            Self::Storage(_) => "storage_unavailable",
        }
    }
}

const MALFORMED_NDJSON: IngestError = IngestError::Malformed("malformed_ndjson");
const RECORD_COUNT_MISMATCH: IngestError = IngestError::Malformed("record_count_mismatch");
const INVALID_HEADER: IngestError = IngestError::Unprocessable("invalid_header");
const INVALID_CURSOR: IngestError = IngestError::Unprocessable("invalid_cursor");
const INVALID_RECORD: IngestError = IngestError::Unprocessable("invalid_record");

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

/// A decoded, validated batch and the SHA-256 of its decoded entity, ready to apply.
#[derive(Debug)]
pub struct PreparedBatch {
    batch: AppendBatch,
    body_sha256: Vec<u8>,
}

/// The CPU-bound half of ingest (decode up to 4 MiB, frame, validate, hash). Synchronous so the
/// caller can run it off the async workers.
pub fn prepare(coding: ContentCoding, wire: Vec<u8>) -> Result<PreparedBatch, IngestError> {
    let entity = decode_entity(coding, wire)?;
    let batch = parse_append_batch(&entity)?;
    Ok(PreparedBatch {
        batch,
        body_sha256: Sha256::digest(&entity).to_vec(),
    })
}

/// Applies a prepared batch idempotently: replays the ledger ack for a byte-identical retry,
/// rejects a conflicting `batchId`, or upserts the records and records the ack.
pub async fn apply(pool: &SqlitePool, prepared: PreparedBatch) -> Result<Accepted, IngestError> {
    let PreparedBatch { batch, body_sha256 } = prepared;

    // IMMEDIATE takes the write lock up front, so concurrent retries of one batch serialize on
    // the ledger instead of both applying it.
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    // The ledger is scoped to the current receiverStateId: rotating it starts a new idempotency
    // generation in which earlier acks are no longer replayed. Rows from older generations are
    // ignored, not discarded; whatever rotates the ID must delete them (the protocol requires
    // rotation to discard old acks).
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
    if let Some((stored_sha256, ack)) = prior {
        // Dropping the transaction rolls it back; nothing was written.
        return if stored_sha256 == body_sha256.as_slice() {
            Ok(Accepted {
                ack,
                stream: batch.stream.name,
                records: batch.records.len(),
                replayed: true,
            })
        } else {
            Err(IngestError::Conflict)
        };
    }

    for record in &batch.records {
        let mut query = sqlx::query(batch.stream.upsert_sql)
            .bind(&batch.source_id)
            .bind(&batch.device_id);
        for value in &record.key {
            query = match value {
                KeyValue::Integer(value) => query.bind(*value),
                KeyValue::Text(value) => query.bind(value.as_str()),
            };
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
        query.execute(&mut *tx).await?;
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

/// A fully validated append batch.
#[derive(Debug)]
pub struct AppendBatch {
    protocol_version: String,
    batch_id: String,
    source_id: String,
    device_id: String,
    stream: &'static AppendStream,
    end_cursor: Cursor,
    records: Vec<Record>,
}

impl AppendBatch {
    /// The acknowledgement: every member echoes the request, `keySha256` verbatim.
    fn ack(&self) -> String {
        serde_json::json!({
            "protocolVersion": self.protocol_version,
            "batchId": self.batch_id,
            "stream": self.stream.name,
            "deviceId": self.device_id,
            "endCursor": {
                "rowId": self.end_cursor.row_id,
                "keySha256": self.end_cursor.key_sha256,
            },
            "acceptedRows": self.records.len(),
            "status": "accepted",
        })
        .to_string()
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Cursor {
    row_id: i64,
    key_sha256: String,
}

#[derive(Debug)]
struct Record {
    key: Vec<KeyValue>,
    data: Vec<DataValue>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum KeyValue {
    Integer(i64),
    Text(String),
}

#[derive(Debug, PartialEq)]
enum DataValue {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Boolean(bool),
}

/// Frames and validates a decoded NDJSON entity as an append batch.
pub fn parse_append_batch(entity: &[u8]) -> Result<AppendBatch, IngestError> {
    // Every line, including the last, ends with LF; there is no trailing material.
    let content = entity.strip_suffix(b"\n").ok_or(MALFORMED_NDJSON)?;
    let mut lines = content.split(|byte| *byte == b'\n');
    let header_line = object_line(lines.next().unwrap_or_default())?;
    let header = parse_header(header_line)?;

    let expected = header.record_count as usize;
    let mut records = Vec::with_capacity(expected);
    let mut keys = HashSet::with_capacity(expected);
    for line in lines {
        let line = object_line(line)?;
        if records.len() == expected {
            return Err(RECORD_COUNT_MISMATCH);
        }
        let record = parse_record(header.stream, line)?;
        if !keys.insert(record.key.clone()) {
            return Err(IngestError::Unprocessable("duplicate_key"));
        }
        records.push(record);
    }
    if records.len() != expected {
        return Err(RECORD_COUNT_MISMATCH);
    }

    Ok(AppendBatch {
        protocol_version: header.protocol_version,
        batch_id: header.batch_id,
        source_id: header.source_id,
        device_id: header.device_id,
        stream: header.stream,
        end_cursor: header.end_cursor,
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
    window: Option<IgnoredAny>,
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
fn presence<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<IgnoredAny>, D::Error> {
    IgnoredAny::deserialize(deserializer).map(Some)
}

struct Header {
    protocol_version: String,
    batch_id: String,
    source_id: String,
    device_id: String,
    stream: &'static AppendStream,
    record_count: u32,
    end_cursor: Cursor,
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
    let stream = registry::append_stream(&header.stream)
        .ok_or(IngestError::Unprocessable("unsupported_stream"))?;
    if header.delivery != "append" || header.window.is_some() {
        return Err(INVALID_HEADER);
    }
    if !is_canonical_uuid(&header.batch_id)
        || !is_canonical_uuid(&header.source_id)
        || header.device_id.is_empty()
    {
        return Err(INVALID_HEADER);
    }
    if header.record_count == 0 {
        return Err(IngestError::Unprocessable("empty_batch"));
    }
    if header.record_count > MAX_RECORDS {
        return Err(INVALID_HEADER);
    }
    let end_cursor = header.end_cursor.ok_or(INVALID_CURSOR)?;
    if !cursors_are_valid(
        header.start_cursor.as_ref(),
        &end_cursor,
        header.record_count,
    ) {
        return Err(INVALID_CURSOR);
    }

    Ok(Header {
        protocol_version: header.protocol_version,
        batch_id: header.batch_id,
        source_id: header.source_id,
        device_id: header.device_id,
        stream,
        record_count: header.record_count,
        end_cursor,
    })
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

fn parse_record(stream: &AppendStream, line: &[u8]) -> Result<Record, IngestError> {
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
