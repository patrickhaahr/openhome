-- Append ingest for the NOOP push endpoint (lives in noop.db, not app.db).
--
-- batch_ledger remembers every accepted batch under (sourceId, deviceId, batchId), scoped to the
-- receiverStateId that accepted it. A byte-identical retry replays the stored ack; a different
-- body under the same batchId is a conflict. Scoping by receiver_state_id means rotating the
-- receiverStateId starts a new idempotency generation: older acks are no longer consulted, so
-- baseline batches are applied again instead of being short-circuited.
CREATE TABLE batch_ledger (
    receiver_state_id TEXT NOT NULL,
    source_id TEXT NOT NULL,
    device_id TEXT NOT NULL,
    batch_id TEXT NOT NULL,
    stream TEXT NOT NULL,
    body_sha256 BLOB NOT NULL,
    ack TEXT NOT NULL,
    accepted_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    PRIMARY KEY (receiver_state_id, source_id, device_id, batch_id)
) STRICT, WITHOUT ROWID;

-- One typed table per v1 append stream. Every row is scoped by (source_id, device_id) plus the
-- stream's natural key, so two installations reporting the same strap never overwrite each other.
-- Booleans are stored as 0/1 integers.

CREATE TABLE hr_sample (
    source_id TEXT NOT NULL,
    device_id TEXT NOT NULL,
    ts INTEGER NOT NULL,
    bpm INTEGER NOT NULL,
    PRIMARY KEY (source_id, device_id, ts)
) STRICT, WITHOUT ROWID;

CREATE TABLE rr_interval (
    source_id TEXT NOT NULL,
    device_id TEXT NOT NULL,
    ts INTEGER NOT NULL,
    rr_ms INTEGER NOT NULL,
    seq INTEGER NOT NULL,
    ord INTEGER,
    src_channel INTEGER,
    ts_suspect INTEGER,
    PRIMARY KEY (source_id, device_id, ts, rr_ms, seq)
) STRICT, WITHOUT ROWID;

CREATE TABLE event (
    source_id TEXT NOT NULL,
    device_id TEXT NOT NULL,
    ts INTEGER NOT NULL,
    kind TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    PRIMARY KEY (source_id, device_id, ts, kind)
) STRICT, WITHOUT ROWID;

CREATE TABLE battery (
    source_id TEXT NOT NULL,
    device_id TEXT NOT NULL,
    ts INTEGER NOT NULL,
    soc REAL,
    mv INTEGER,
    charging INTEGER CHECK (charging IN (0, 1)),
    PRIMARY KEY (source_id, device_id, ts)
) STRICT, WITHOUT ROWID;

CREATE TABLE spo2_sample (
    source_id TEXT NOT NULL,
    device_id TEXT NOT NULL,
    ts INTEGER NOT NULL,
    red INTEGER NOT NULL,
    ir INTEGER NOT NULL,
    PRIMARY KEY (source_id, device_id, ts)
) STRICT, WITHOUT ROWID;

CREATE TABLE skin_temp_sample (
    source_id TEXT NOT NULL,
    device_id TEXT NOT NULL,
    ts INTEGER NOT NULL,
    raw INTEGER NOT NULL,
    -- REAL: the protocol does not type the aux readings as integers (see registry.rs).
    aux1_raw REAL,
    aux2_raw REAL,
    PRIMARY KEY (source_id, device_id, ts)
) STRICT, WITHOUT ROWID;

CREATE TABLE resp_sample (
    source_id TEXT NOT NULL,
    device_id TEXT NOT NULL,
    ts INTEGER NOT NULL,
    raw INTEGER NOT NULL,
    PRIMARY KEY (source_id, device_id, ts)
) STRICT, WITHOUT ROWID;

CREATE TABLE gravity_sample (
    source_id TEXT NOT NULL,
    device_id TEXT NOT NULL,
    ts INTEGER NOT NULL,
    x REAL NOT NULL,
    y REAL NOT NULL,
    z REAL NOT NULL,
    dyn_accel REAL,
    PRIMARY KEY (source_id, device_id, ts)
) STRICT, WITHOUT ROWID;
