-- Rebuild so legacy completed rows receive a date before the final constraint is checked.
CREATE TABLE watchlist_entries_with_completion (
    id TEXT PRIMARY KEY NOT NULL,
    owner_id TEXT NOT NULL REFERENCES household_users(id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN ('movie', 'series')),
    title TEXT NOT NULL CHECK (length(trim(title)) BETWEEN 1 AND 500),
    status TEXT NOT NULL DEFAULT 'planned' CHECK (status IN ('planned', 'watching', 'completed')),
    date_added TEXT NOT NULL,
    completion_date TEXT CHECK (
        (status = 'completed' AND completion_date IS NOT NULL AND completion_date >= date_added)
        OR (status != 'completed' AND completion_date IS NULL)
    )
);
INSERT INTO watchlist_entries_with_completion (id, owner_id, kind, title, status, date_added, completion_date)
SELECT id, owner_id, kind, title, status, date_added,
    CASE WHEN status = 'completed' THEN date_added ELSE NULL END
FROM watchlist_entries;
-- Addition receipts deliberately have no foreign key and survive this replacement.
DROP TABLE watchlist_entries;
ALTER TABLE watchlist_entries_with_completion RENAME TO watchlist_entries;
CREATE INDEX watchlist_entries_owner ON watchlist_entries(owner_id, date_added, id);
