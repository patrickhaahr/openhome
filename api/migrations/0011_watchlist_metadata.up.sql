ALTER TABLE watchlist_entries ADD COLUMN tmdb_id INTEGER CHECK (tmdb_id > 0 AND tmdb_id <= 9007199254740991);
ALTER TABLE watchlist_entries ADD COLUMN overview TEXT;
ALTER TABLE watchlist_entries ADD COLUMN poster_path TEXT;
ALTER TABLE watchlist_entries ADD COLUMN release_date TEXT;
CREATE UNIQUE INDEX watchlist_provider_identity ON watchlist_entries(owner_id, kind, tmdb_id) WHERE tmdb_id IS NOT NULL;
-- The selected provider identity is part of an immutable addition intention.
ALTER TABLE watchlist_addition_receipts ADD COLUMN tmdb_id INTEGER;
