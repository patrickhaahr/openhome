CREATE TABLE watchlist_entries (
    id TEXT PRIMARY KEY NOT NULL,
    owner_id TEXT NOT NULL REFERENCES household_users(id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN ('movie', 'series')),
    title TEXT NOT NULL CHECK (length(trim(title)) BETWEEN 1 AND 500),
    status TEXT NOT NULL DEFAULT 'planned' CHECK (status IN ('planned', 'watching', 'completed')),
    date_added TEXT NOT NULL
);
CREATE INDEX watchlist_entries_owner ON watchlist_entries(owner_id, date_added, id);

-- No foreign key: accepted intentions remain retired after entry/owner deletion.
CREATE TABLE watchlist_addition_receipts (
    request_id TEXT PRIMARY KEY NOT NULL,
    entry_id TEXT NOT NULL,
    owner_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    title TEXT NOT NULL
);
