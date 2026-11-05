-- One receipt per accepted Exercise create that carried a request_id, holding its normalized
-- arguments, so a retry after a lost response finds the Exercise it already created rather than
-- failing on its unique name. Deleting the Exercise clears exercise_id (ids can be reused) but keeps
-- the receipt, so an old retry cannot recreate it.
CREATE TABLE exercise_receipts (
    request_id TEXT PRIMARY KEY NOT NULL,
    exercise_id INTEGER REFERENCES exercises(id) ON DELETE SET NULL,
    request TEXT NOT NULL
);
