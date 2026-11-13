-- When the daily Card Price refresh is next due, as Unix seconds. A restart reads it, so the
-- catalogue is asked once a day however often the process starts. A pass pushes it a day ahead
-- when it starts, not when it ends: a crash mid-pass waits for the next day instead of sending
-- every owned code again on each restart. A pass the database failed pulls it back to a short
-- retry.
CREATE TABLE price_refresh_state (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    next_due_at INTEGER NOT NULL CHECK (next_due_at >= 0)
);
