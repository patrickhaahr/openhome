-- The previous application can decode only planned entries; downgrade discards viewing progress.
UPDATE watchlist_entries SET status = 'planned', completion_date = NULL;
ALTER TABLE watchlist_entries DROP COLUMN completion_date;
