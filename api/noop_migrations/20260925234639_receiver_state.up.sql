-- Receiver state for the NOOP push endpoint (lives in noop.db, not app.db).
-- A single row holds the receiverStateId advertised in capabilities. The API
-- seeds it on first start; rotating it signals that continuity with previously
-- acknowledged data was lost (for example after restoring a stale backup).
CREATE TABLE receiver_state (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    receiver_state_id TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
