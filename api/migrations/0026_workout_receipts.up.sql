-- One receipt per accepted Workout create that carried a request_id, holding its normalized
-- arguments, so a retry after a lost response finds the Workout it already created. Deleting the
-- Workout clears workout_id (ids can be reused) but keeps the receipt, so an old retry cannot
-- recreate it.
CREATE TABLE workout_receipts (
    request_id TEXT PRIMARY KEY NOT NULL,
    workout_id INTEGER REFERENCES workouts(id) ON DELETE SET NULL,
    request TEXT NOT NULL
);
