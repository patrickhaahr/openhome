//! The v1 stream registry (`.noop/PUSH_PROTOCOL.md`, "Version 1 stream registry").
//!
//! The registry is explicit and finite: every stream lists its delivery mode, its natural-key
//! columns (excluding the batch-scoped `deviceId`), its exported `data` members with their wire
//! types, and the upsert that stores a record in the stream's typed table in `noop.db`.
//! Replace-window streams also carry the SQL that finds and deletes rows absent from a complete
//! replacement.

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

/// How a stream is delivered. Fixed by the registry; a header naming the other mode is invalid.
#[derive(Debug)]
pub enum Delivery {
    /// Idempotent upserts guarded by insertion cursors.
    Append,
    /// Authoritative replacement of every row in a half-open window.
    ReplaceWindow(ReplaceWindow),
}

/// The column a replace-window stream's window bounds apply to. It is always the first key
/// column, whose name equals the selector's wire name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Selector {
    /// `YYYY-MM-DD` bounds over the `day` key.
    Day,
    /// Integer Unix-second bounds over the `startTs` key.
    StartTs,
}

impl Selector {
    pub fn wire_name(self) -> &'static str {
        match self {
            Self::Day => "day",
            Self::StartTs => "startTs",
        }
    }
}

#[derive(Debug)]
pub struct ReplaceWindow {
    pub selector: Selector,
    /// Binds `source_id`, `device_id`, `startInclusive`, `endExclusive`; returns the key columns
    /// (registry order) of every stored row in that scope and window.
    pub window_keys_sql: &'static str,
    /// Binds `source_id`, `device_id`, then the key columns in registry order.
    pub delete_sql: &'static str,
}

#[derive(Debug)]
pub struct Stream {
    pub name: &'static str,
    pub delivery: Delivery,
    /// Natural-key columns in registry order. Key columns are never nullable.
    pub key: &'static [Column],
    pub data: &'static [Column],
    /// Binds `source_id`, `device_id`, then the key columns and the data columns in registry order.
    pub upsert_sql: &'static str,
}

impl Stream {
    pub fn replace_window(&self) -> Option<&ReplaceWindow> {
        match &self.delivery {
            Delivery::Append => None,
            Delivery::ReplaceWindow(window) => Some(window),
        }
    }
}

const TS_KEY: &[Column] = &[required("ts", Integer)];

/// The complete v1 registry, in wire order: the 8 append streams, then the 4 mutable ones.
pub const STREAMS: [Stream; 12] = [
    Stream {
        name: "hrSample",
        delivery: Delivery::Append,
        key: TS_KEY,
        data: &[required("bpm", Integer)],
        upsert_sql: "INSERT INTO hr_sample (source_id, device_id, ts, bpm) VALUES (?, ?, ?, ?) \
             ON CONFLICT (source_id, device_id, ts) DO UPDATE SET bpm = excluded.bpm",
    },
    Stream {
        name: "rrInterval",
        delivery: Delivery::Append,
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
    Stream {
        name: "event",
        delivery: Delivery::Append,
        key: &[required("ts", Integer), required("kind", Text)],
        data: &[required("payloadJSON", Text)],
        upsert_sql: "INSERT INTO event (source_id, device_id, ts, kind, payload_json) \
             VALUES (?, ?, ?, ?, ?) \
             ON CONFLICT (source_id, device_id, ts, kind) DO UPDATE SET \
             payload_json = excluded.payload_json",
    },
    Stream {
        name: "battery",
        delivery: Delivery::Append,
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
    Stream {
        name: "spo2Sample",
        delivery: Delivery::Append,
        key: TS_KEY,
        data: &[required("red", Integer), required("ir", Integer)],
        upsert_sql: "INSERT INTO spo2_sample (source_id, device_id, ts, red, ir) \
             VALUES (?, ?, ?, ?, ?) \
             ON CONFLICT (source_id, device_id, ts) DO UPDATE SET \
             red = excluded.red, ir = excluded.ir",
    },
    Stream {
        name: "skinTempSample",
        delivery: Delivery::Append,
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
    Stream {
        name: "respSample",
        delivery: Delivery::Append,
        key: TS_KEY,
        data: &[required("raw", Integer)],
        upsert_sql: "INSERT INTO resp_sample (source_id, device_id, ts, raw) VALUES (?, ?, ?, ?) \
             ON CONFLICT (source_id, device_id, ts) DO UPDATE SET raw = excluded.raw",
    },
    Stream {
        name: "gravitySample",
        delivery: Delivery::Append,
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
    // Mutable streams. Per the protocol, timestamps and counts are integers and every metric or
    // measurement is any finite number (so `restingHr` accepts `58` or `58.0`); only `endTs`,
    // `source`, `userEdited` and `answeredYes` are required.
    Stream {
        name: "dailyMetric",
        delivery: Delivery::ReplaceWindow(ReplaceWindow {
            selector: Selector::Day,
            window_keys_sql: "SELECT day FROM daily_metric \
                 WHERE source_id = ? AND device_id = ? AND day >= ? AND day < ?",
            delete_sql: "DELETE FROM daily_metric WHERE source_id = ? AND device_id = ? AND day = ?",
        }),
        key: &[required("day", Text)],
        data: &[
            nullable("totalSleepMin", Real),
            nullable("efficiency", Real),
            nullable("deepMin", Real),
            nullable("remMin", Real),
            nullable("lightMin", Real),
            nullable("disturbances", Integer),
            nullable("restingHr", Real),
            nullable("avgHrv", Real),
            nullable("recovery", Real),
            nullable("strain", Real),
            nullable("exerciseCount", Integer),
            nullable("spo2Pct", Real),
            nullable("skinTempDevC", Real),
            nullable("respRateBpm", Real),
            nullable("steps", Integer),
            nullable("activeKcalEst", Real),
            nullable("spo2Red", Real),
            nullable("spo2Ir", Real),
        ],
        upsert_sql: "INSERT INTO daily_metric (source_id, device_id, day, total_sleep_min, \
             efficiency, deep_min, rem_min, light_min, disturbances, resting_hr, avg_hrv, recovery, \
             strain, exercise_count, spo2_pct, skin_temp_dev_c, resp_rate_bpm, steps, \
             active_kcal_est, spo2_red, spo2_ir) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) \
             ON CONFLICT (source_id, device_id, day) DO UPDATE SET \
             total_sleep_min = excluded.total_sleep_min, efficiency = excluded.efficiency, \
             deep_min = excluded.deep_min, rem_min = excluded.rem_min, \
             light_min = excluded.light_min, disturbances = excluded.disturbances, \
             resting_hr = excluded.resting_hr, avg_hrv = excluded.avg_hrv, \
             recovery = excluded.recovery, strain = excluded.strain, \
             exercise_count = excluded.exercise_count, spo2_pct = excluded.spo2_pct, \
             skin_temp_dev_c = excluded.skin_temp_dev_c, resp_rate_bpm = excluded.resp_rate_bpm, \
             steps = excluded.steps, active_kcal_est = excluded.active_kcal_est, \
             spo2_red = excluded.spo2_red, spo2_ir = excluded.spo2_ir",
    },
    Stream {
        name: "sleepSession",
        delivery: Delivery::ReplaceWindow(ReplaceWindow {
            selector: Selector::StartTs,
            window_keys_sql: "SELECT start_ts FROM sleep_session \
                 WHERE source_id = ? AND device_id = ? AND start_ts >= ? AND start_ts < ?",
            delete_sql: "DELETE FROM sleep_session \
                 WHERE source_id = ? AND device_id = ? AND start_ts = ?",
        }),
        key: &[required("startTs", Integer)],
        data: &[
            required("endTs", Integer),
            nullable("efficiency", Real),
            nullable("restingHr", Real),
            nullable("avgHrv", Real),
            nullable("stagesJSON", Text),
            required("userEdited", Boolean),
            nullable("startTsAdjusted", Integer),
            nullable("motionJSON", Text),
            nullable("sleepStateJSON", Text),
            nullable("stagingSparse", Boolean),
        ],
        upsert_sql: "INSERT INTO sleep_session (source_id, device_id, start_ts, end_ts, \
             efficiency, resting_hr, avg_hrv, stages_json, user_edited, start_ts_adjusted, \
             motion_json, sleep_state_json, staging_sparse) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) \
             ON CONFLICT (source_id, device_id, start_ts) DO UPDATE SET \
             end_ts = excluded.end_ts, efficiency = excluded.efficiency, \
             resting_hr = excluded.resting_hr, avg_hrv = excluded.avg_hrv, \
             stages_json = excluded.stages_json, user_edited = excluded.user_edited, \
             start_ts_adjusted = excluded.start_ts_adjusted, motion_json = excluded.motion_json, \
             sleep_state_json = excluded.sleep_state_json, \
             staging_sparse = excluded.staging_sparse",
    },
    Stream {
        name: "workout",
        delivery: Delivery::ReplaceWindow(ReplaceWindow {
            selector: Selector::StartTs,
            window_keys_sql: "SELECT start_ts, sport FROM workout \
                 WHERE source_id = ? AND device_id = ? AND start_ts >= ? AND start_ts < ?",
            delete_sql: "DELETE FROM workout \
                 WHERE source_id = ? AND device_id = ? AND start_ts = ? AND sport = ?",
        }),
        key: &[required("startTs", Integer), required("sport", Text)],
        data: &[
            required("endTs", Integer),
            required("source", Text),
            nullable("durationS", Real),
            nullable("energyKcal", Real),
            nullable("avgHr", Real),
            nullable("maxHr", Real),
            nullable("strain", Real),
            nullable("distanceM", Real),
            nullable("zonesJSON", Text),
            nullable("notes", Text),
            nullable("routePolyline", Text),
            nullable("steps", Integer),
        ],
        upsert_sql: "INSERT INTO workout (source_id, device_id, start_ts, sport, end_ts, source, \
             duration_s, energy_kcal, avg_hr, max_hr, strain, distance_m, zones_json, notes, \
             route_polyline, steps) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) \
             ON CONFLICT (source_id, device_id, start_ts, sport) DO UPDATE SET \
             end_ts = excluded.end_ts, source = excluded.source, \
             duration_s = excluded.duration_s, energy_kcal = excluded.energy_kcal, \
             avg_hr = excluded.avg_hr, max_hr = excluded.max_hr, strain = excluded.strain, \
             distance_m = excluded.distance_m, zones_json = excluded.zones_json, \
             notes = excluded.notes, route_polyline = excluded.route_polyline, \
             steps = excluded.steps",
    },
    Stream {
        name: "journal",
        delivery: Delivery::ReplaceWindow(ReplaceWindow {
            selector: Selector::Day,
            window_keys_sql: "SELECT day, question FROM journal \
                 WHERE source_id = ? AND device_id = ? AND day >= ? AND day < ?",
            delete_sql: "DELETE FROM journal \
                 WHERE source_id = ? AND device_id = ? AND day = ? AND question = ?",
        }),
        key: &[required("day", Text), required("question", Text)],
        data: &[
            required("answeredYes", Boolean),
            nullable("notes", Text),
            nullable("numericValue", Real),
        ],
        upsert_sql: "INSERT INTO journal (source_id, device_id, day, question, answered_yes, \
             notes, numeric_value) VALUES (?, ?, ?, ?, ?, ?, ?) \
             ON CONFLICT (source_id, device_id, day, question) DO UPDATE SET \
             answered_yes = excluded.answered_yes, notes = excluded.notes, \
             numeric_value = excluded.numeric_value",
    },
];

/// Looks up a stream by its wire name.
pub fn stream(name: &str) -> Option<&'static Stream> {
    STREAMS.iter().find(|stream| stream.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::noop_push::V1_STREAMS;

    #[test]
    fn registry_matches_the_advertised_v1_streams() {
        let names: Vec<_> = STREAMS.iter().map(|stream| stream.name).collect();
        assert_eq!(names, V1_STREAMS);
        let append: Vec<_> = STREAMS
            .iter()
            .filter(|stream| stream.replace_window().is_none())
            .map(|stream| stream.name)
            .collect();
        assert_eq!(append, V1_STREAMS[..8]);
    }

    #[test]
    fn upserts_bind_scope_key_and_data_columns() {
        for stream in &STREAMS {
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
    fn replace_window_selector_is_the_first_key_column() {
        for stream in &STREAMS {
            let Some(window) = stream.replace_window() else {
                continue;
            };
            let first = &stream.key[0];
            assert_eq!(first.name, window.selector.wire_name(), "{}", stream.name);
            let expected_ty = match window.selector {
                Selector::Day => Text,
                Selector::StartTs => Integer,
            };
            assert_eq!(first.ty, expected_ty, "{}", stream.name);
            assert_eq!(window.window_keys_sql.matches('?').count(), 4);
            assert_eq!(
                window.delete_sql.matches('?').count(),
                2 + stream.key.len(),
                "{}",
                stream.name
            );
        }
    }

    #[test]
    fn required_mutable_members_match_the_protocol() {
        let required: Vec<_> = STREAMS
            .iter()
            .filter(|stream| stream.replace_window().is_some())
            .flat_map(|stream| {
                stream
                    .data
                    .iter()
                    .filter(|column| !column.nullable)
                    .map(move |column| (stream.name, column.name, column.ty))
            })
            .collect();
        assert_eq!(
            required,
            [
                ("sleepSession", "endTs", Integer),
                ("sleepSession", "userEdited", Boolean),
                ("workout", "endTs", Integer),
                ("workout", "source", Text),
                ("journal", "answeredYes", Boolean),
            ]
        );
    }

    #[test]
    fn lookup_covers_all_streams_and_nothing_else() {
        assert!(stream("dailyMetric").is_some_and(|s| s.replace_window().is_some()));
        assert!(stream("hrSample").is_some_and(|s| s.replace_window().is_none()));
        assert!(stream("ppgHrSample").is_none());
    }
}
