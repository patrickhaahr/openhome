-- The newest edit revision an offline client has applied to the Profile, so its replayed or late edits
-- cannot restore an older one. Null until a client sends one; edits without one leave it alone.
ALTER TABLE profile ADD COLUMN client_revision INTEGER;
