ALTER TABLE watchlist_addition_receipts DROP COLUMN tmdb_id;
DROP INDEX watchlist_provider_identity;
ALTER TABLE watchlist_entries DROP COLUMN release_date;
ALTER TABLE watchlist_entries DROP COLUMN poster_path;
ALTER TABLE watchlist_entries DROP COLUMN overview;
ALTER TABLE watchlist_entries DROP COLUMN tmdb_id;
