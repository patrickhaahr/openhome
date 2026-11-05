-- The newest edit revision an offline client has applied to this Workout, so its replayed or late
-- edits cannot restore an older one. Null until a client sends one; edits without one leave it alone.
ALTER TABLE workouts ADD COLUMN client_revision INTEGER;
