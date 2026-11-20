use std::collections::HashSet;

use chrono::NaiveDate;
use serde::de::{self, IgnoredAny, MapAccess, Visitor};
use serde::{Deserialize, Deserializer};
use serde_json::{Map, Value};
use uuid::Uuid;

use super::super::SUPPORTED_VERSIONS;
use super::super::registry::{self, Column, ColumnType, Selector, Stream};

use super::{
    Batch, BatchDelivery, Bounds, Cursor, DUPLICATE_KEY, DataValue, EMPTY_BATCH, INVALID_CURSOR,
    INVALID_HEADER, INVALID_RECORD, INVALID_WINDOW, IngestError, KeyValue, MALFORMED_NDJSON,
    MAX_RECORDS, RECORD_COUNT_MISMATCH, Record, Window,
};

pub fn parse_batch(entity: &[u8]) -> Result<Batch, IngestError> {
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

fn object_line(line: &[u8]) -> Result<&[u8], IngestError> {
    if line.first() == Some(&b'{') && line.last() == Some(&b'}') {
        Ok(line)
    } else {
        Err(MALFORMED_NDJSON)
    }
}

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

fn required_nullable<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::deserialize(deserializer)
}

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

fn is_day(value: &str) -> bool {
    value.len() == 10
        && NaiveDate::parse_from_str(value, "%Y-%m-%d")
            .is_ok_and(|date| date.format("%Y-%m-%d").to_string() == value)
}

fn is_canonical_uuid(value: &str) -> bool {
    Uuid::parse_str(value).is_ok_and(|uuid| uuid.hyphenated().to_string() == value)
}

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
    end.row_id - floor >= i64::from(record_count)
}

#[derive(Deserialize)]
struct RawRecord {
    #[serde(rename = "type")]
    kind: String,
    key: Members,
    data: Members,
}

struct Members(Map<String, Value>);

impl<'de> Deserialize<'de> for Members {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct MembersVisitor;

        impl<'de> Visitor<'de> for MembersVisitor {
            type Value = Members;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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
