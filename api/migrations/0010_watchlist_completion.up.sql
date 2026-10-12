-- Existing entries are planned; migrated completed rows receive their addition date.
ALTER TABLE watchlist_entries ADD COLUMN completion_date TEXT
    CHECK ((status = 'completed' AND completion_date IS NOT NULL AND completion_date >= date_added)
        OR (status != 'completed' AND completion_date IS NULL));
UPDATE watchlist_entries SET completion_date = date_added WHERE status = 'completed';
