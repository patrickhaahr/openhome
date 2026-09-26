-- Replace-window ingest for the NOOP push endpoint (lives in noop.db, not app.db).
--
-- One typed table per v1 mutable stream, scoped by (source_id, device_id) plus the stream's
-- natural key like the append tables. Booleans are stored as 0/1 integers; `day` is YYYY-MM-DD.
-- sleep_session and workout keep a rowid because their JSON/polyline text can make rows large,
-- which WITHOUT ROWID tables handle poorly.

CREATE TABLE daily_metric (
    source_id TEXT NOT NULL,
    device_id TEXT NOT NULL,
    day TEXT NOT NULL,
    total_sleep_min REAL,
    efficiency REAL,
    deep_min REAL,
    rem_min REAL,
    light_min REAL,
    disturbances INTEGER,
    resting_hr REAL,
    avg_hrv REAL,
    recovery REAL,
    strain REAL,
    exercise_count INTEGER,
    spo2_pct REAL,
    skin_temp_dev_c REAL,
    resp_rate_bpm REAL,
    steps INTEGER,
    active_kcal_est REAL,
    spo2_red REAL,
    spo2_ir REAL,
    PRIMARY KEY (source_id, device_id, day)
) STRICT, WITHOUT ROWID;

CREATE TABLE sleep_session (
    source_id TEXT NOT NULL,
    device_id TEXT NOT NULL,
    start_ts INTEGER NOT NULL,
    end_ts INTEGER NOT NULL,
    efficiency REAL,
    resting_hr REAL,
    avg_hrv REAL,
    stages_json TEXT,
    user_edited INTEGER NOT NULL CHECK (user_edited IN (0, 1)),
    start_ts_adjusted INTEGER,
    motion_json TEXT,
    sleep_state_json TEXT,
    staging_sparse INTEGER CHECK (staging_sparse IN (0, 1)),
    PRIMARY KEY (source_id, device_id, start_ts)
) STRICT;

CREATE TABLE workout (
    source_id TEXT NOT NULL,
    device_id TEXT NOT NULL,
    start_ts INTEGER NOT NULL,
    sport TEXT NOT NULL,
    end_ts INTEGER NOT NULL,
    source TEXT NOT NULL,
    duration_s REAL,
    energy_kcal REAL,
    avg_hr REAL,
    max_hr REAL,
    strain REAL,
    distance_m REAL,
    zones_json TEXT,
    notes TEXT,
    route_polyline TEXT,
    steps INTEGER,
    PRIMARY KEY (source_id, device_id, start_ts, sport)
) STRICT;

CREATE TABLE journal (
    source_id TEXT NOT NULL,
    device_id TEXT NOT NULL,
    day TEXT NOT NULL,
    question TEXT NOT NULL,
    answered_yes INTEGER NOT NULL CHECK (answered_yes IN (0, 1)),
    notes TEXT,
    numeric_value REAL,
    PRIMARY KEY (source_id, device_id, day, question)
) STRICT, WITHOUT ROWID;

-- Replacement generations. Every accepted replace_window part belongs to one replacement
-- (replacementId) with a fixed stream, window and part count; reusing the replacementId with
-- different metadata is a conflict. `state` is `staging` until every part is present,
-- `applied` once the atomic apply committed, or `superseded` when a newer generation for the same
-- (source, device, stream) arrived first; superseded rows are the generation fences that turn
-- late parts into 409s. Like batch_ledger, everything is scoped to the receiverStateId, so a
-- rotation starts with no staging and no fences.
CREATE TABLE replacement (
    receiver_state_id TEXT NOT NULL,
    source_id TEXT NOT NULL,
    device_id TEXT NOT NULL,
    replacement_id TEXT NOT NULL,
    stream TEXT NOT NULL,
    selector TEXT NOT NULL,
    -- Canonical text of the bounds: the day itself or the decimal Unix-second value.
    start_inclusive TEXT NOT NULL,
    end_exclusive TEXT NOT NULL,
    parts INTEGER NOT NULL CHECK (parts >= 1),
    state TEXT NOT NULL CHECK (state IN ('staging', 'applied', 'superseded')),
    PRIMARY KEY (receiver_state_id, source_id, device_id, replacement_id)
) STRICT, WITHOUT ROWID;

-- The latest generation observed per (source, device, stream). At most one generation per scope
-- is `staging`, and it is always the current one.
CREATE TABLE replacement_scope (
    receiver_state_id TEXT NOT NULL,
    source_id TEXT NOT NULL,
    device_id TEXT NOT NULL,
    stream TEXT NOT NULL,
    current_replacement_id TEXT NOT NULL,
    PRIMARY KEY (receiver_state_id, source_id, device_id, stream)
) STRICT, WITHOUT ROWID;

-- One row per accepted part. `batch_id` is kept for the life of the replacement so reusing a
-- part number with a different batch is detected; `entity` holds the part's decoded NDJSON only
-- while it is staged and is cleared once the replacement is applied or superseded.
CREATE TABLE replacement_part (
    receiver_state_id TEXT NOT NULL,
    source_id TEXT NOT NULL,
    device_id TEXT NOT NULL,
    replacement_id TEXT NOT NULL,
    part INTEGER NOT NULL CHECK (part >= 1),
    batch_id TEXT NOT NULL,
    entity BLOB,
    PRIMARY KEY (receiver_state_id, source_id, device_id, replacement_id, part)
) STRICT;

-- Operators rotate the ID directly in receiver_state. Clear protocol metadata in that same
-- transaction so no old acknowledgements, staged health payloads or generation fences survive.
-- The stream tables retain their health records for the next baseline to reconcile.
CREATE TRIGGER receiver_state_rotation
AFTER UPDATE OF receiver_state_id ON receiver_state
WHEN OLD.receiver_state_id <> NEW.receiver_state_id
BEGIN
    DELETE FROM replacement_part;
    DELETE FROM replacement_scope;
    DELETE FROM replacement;
    DELETE FROM batch_ledger;
END;
