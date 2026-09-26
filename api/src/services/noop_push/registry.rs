//! The v1 append-stream registry (`.noop/PUSH_PROTOCOL.md`, "Version 1 stream registry").
//!
//! The registry is explicit and finite: every stream lists its natural-key columns (excluding the
//! batch-scoped `deviceId`), its exported `data` members with their wire types, and the upsert
//! that stores a record in the stream's typed table in `noop.db`.

/// Wire type of one registry column.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColumnType {
    /// A JSON number without fraction or exponent that fits in `i64`.
    Integer,
    /// Any finite JSON number.
    Real,
    Text,
    Boolean,
}

#[derive(Debug)]
pub struct Column {
    pub name: &'static str,
    pub ty: ColumnType,
    pub nullable: bool,
}

const fn required(name: &'static str, ty: ColumnType) -> Column {
    Column {
        name,
        ty,
        nullable: false,
    }
}

const fn nullable(name: &'static str, ty: ColumnType) -> Column {
    Column {
        name,
        ty,
        nullable: true,
    }
}

use ColumnType::{Boolean, Integer, Real, Text};

#[derive(Debug)]
pub struct AppendStream {
    pub name: &'static str,
    /// Natural-key columns in registry order. Key columns are never nullable.
    pub key: &'static [Column],
    pub data: &'static [Column],
    /// Binds `source_id`, `device_id`, then the key columns and the data columns in registry order.
    pub upsert_sql: &'static str,
}

const TS_KEY: &[Column] = &[required("ts", Integer)];

/// The complete v1 append registry, in wire order.
pub const APPEND_STREAMS: [AppendStream; 8] = [
    AppendStream {
        name: "hrSample",
        key: TS_KEY,
        data: &[required("bpm", Integer)],
        upsert_sql: "INSERT INTO hr_sample (source_id, device_id, ts, bpm) VALUES (?, ?, ?, ?) \
             ON CONFLICT (source_id, device_id, ts) DO UPDATE SET bpm = excluded.bpm",
    },
    AppendStream {
        name: "rrInterval",
        key: &[
            required("ts", Integer),
            required("rrMs", Integer),
            required("seq", Integer),
        ],
        data: &[
            nullable("ord", Integer),
            nullable("srcChannel", Integer),
            nullable("tsSuspect", Integer),
        ],
        upsert_sql: "INSERT INTO rr_interval \
             (source_id, device_id, ts, rr_ms, seq, ord, src_channel, ts_suspect) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?) \
             ON CONFLICT (source_id, device_id, ts, rr_ms, seq) DO UPDATE SET \
             ord = excluded.ord, src_channel = excluded.src_channel, \
             ts_suspect = excluded.ts_suspect",
    },
    AppendStream {
        name: "event",
        key: &[required("ts", Integer), required("kind", Text)],
        data: &[required("payloadJSON", Text)],
        upsert_sql: "INSERT INTO event (source_id, device_id, ts, kind, payload_json) \
             VALUES (?, ?, ?, ?, ?) \
             ON CONFLICT (source_id, device_id, ts, kind) DO UPDATE SET \
             payload_json = excluded.payload_json",
    },
    AppendStream {
        name: "battery",
        key: TS_KEY,
        data: &[
            nullable("soc", Real),
            nullable("mv", Integer),
            nullable("charging", Boolean),
        ],
        upsert_sql: "INSERT INTO battery (source_id, device_id, ts, soc, mv, charging) \
             VALUES (?, ?, ?, ?, ?, ?) \
             ON CONFLICT (source_id, device_id, ts) DO UPDATE SET \
             soc = excluded.soc, mv = excluded.mv, charging = excluded.charging",
    },
    AppendStream {
        name: "spo2Sample",
        key: TS_KEY,
        data: &[required("red", Integer), required("ir", Integer)],
        upsert_sql: "INSERT INTO spo2_sample (source_id, device_id, ts, red, ir) \
             VALUES (?, ?, ?, ?, ?) \
             ON CONFLICT (source_id, device_id, ts) DO UPDATE SET \
             red = excluded.red, ir = excluded.ir",
    },
    AppendStream {
        name: "skinTempSample",
        key: TS_KEY,
        data: &[
            required("raw", Integer),
            // Real, not Integer: PUSH_PROTOCOL.md's integer list (`ts`, `rrMs`, `seq`, `bpm`,
            // `red`, `ir`, `raw`) omits the aux columns, DATA_MODEL.md does not document them,
            // and the sender picks the JSON type from the SQLite storage type (PushDao.kt
            // `Cursor.values`), so `12.0` is possible. Any finite number is accepted.
            nullable("aux1Raw", Real),
            nullable("aux2Raw", Real),
        ],
        upsert_sql: "INSERT INTO skin_temp_sample (source_id, device_id, ts, raw, aux1_raw, aux2_raw) \
             VALUES (?, ?, ?, ?, ?, ?) \
             ON CONFLICT (source_id, device_id, ts) DO UPDATE SET \
             raw = excluded.raw, aux1_raw = excluded.aux1_raw, aux2_raw = excluded.aux2_raw",
    },
    AppendStream {
        name: "respSample",
        key: TS_KEY,
        data: &[required("raw", Integer)],
        upsert_sql: "INSERT INTO resp_sample (source_id, device_id, ts, raw) VALUES (?, ?, ?, ?) \
             ON CONFLICT (source_id, device_id, ts) DO UPDATE SET raw = excluded.raw",
    },
    AppendStream {
        name: "gravitySample",
        key: TS_KEY,
        data: &[
            required("x", Real),
            required("y", Real),
            required("z", Real),
            nullable("dynAccel", Real),
        ],
        upsert_sql: "INSERT INTO gravity_sample (source_id, device_id, ts, x, y, z, dyn_accel) \
             VALUES (?, ?, ?, ?, ?, ?, ?) \
             ON CONFLICT (source_id, device_id, ts) DO UPDATE SET \
             x = excluded.x, y = excluded.y, z = excluded.z, dyn_accel = excluded.dyn_accel",
    },
];

/// Looks up an append stream by its wire name.
pub fn append_stream(name: &str) -> Option<&'static AppendStream> {
    APPEND_STREAMS.iter().find(|stream| stream.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::noop_push::V1_STREAMS;

    #[test]
    fn append_registry_matches_the_first_eight_v1_streams() {
        let names: Vec<_> = APPEND_STREAMS.iter().map(|stream| stream.name).collect();
        assert_eq!(names, V1_STREAMS[..8]);
    }

    #[test]
    fn upserts_bind_scope_key_and_data_columns() {
        for stream in &APPEND_STREAMS {
            let placeholders = stream.upsert_sql.matches('?').count();
            assert_eq!(
                placeholders,
                2 + stream.key.len() + stream.data.len(),
                "{}",
                stream.name
            );
            assert!(stream.key.iter().all(|column| !column.nullable));
            assert!(
                stream
                    .key
                    .iter()
                    .all(|column| matches!(column.ty, Integer | Text))
            );
        }
    }

    #[test]
    fn mutable_streams_are_not_append_streams() {
        for name in [
            "dailyMetric",
            "sleepSession",
            "workout",
            "journal",
            "ppgHrSample",
        ] {
            assert!(append_stream(name).is_none(), "{name}");
        }
        assert_eq!(append_stream("hrSample").map(|s| s.name), Some("hrSample"));
    }
}
