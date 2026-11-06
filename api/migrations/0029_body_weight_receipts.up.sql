-- One receipt per accepted Body Weight record that carried a request_id, holding its normalized
-- arguments, so a retry after a lost response finds the record it already created rather than
-- failing on its unique date. A new request_id for a recorded date stays a conflict. Removing the
-- record clears body_weight_id (ids can be reused) but keeps the receipt, so an old retry cannot
-- recreate it.
CREATE TABLE body_weight_receipts (
    request_id TEXT PRIMARY KEY NOT NULL,
    body_weight_id INTEGER REFERENCES body_weight(id) ON DELETE SET NULL,
    request TEXT NOT NULL
);
