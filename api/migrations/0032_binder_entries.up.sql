CREATE TABLE binder_entries (
    id TEXT PRIMARY KEY NOT NULL,
    owner_id TEXT NOT NULL REFERENCES household_users(id) ON DELETE CASCADE,
    card_id TEXT NOT NULL REFERENCES cards(id),
    language TEXT NOT NULL CHECK (language IN ('en', 'ja')),
    quantity INTEGER NOT NULL CHECK (quantity >= 1),
    note TEXT CHECK (note IS NULL OR length(trim(note)) BETWEEN 1 AND 500),
    date_added TEXT NOT NULL CHECK (date_added GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'),
    UNIQUE (owner_id, card_id, language)
);
CREATE INDEX binder_entries_card ON binder_entries(card_id);

-- No foreign key: an accepted addition stays retired after its entry or owner is deleted.
-- `result` is the original response, so a replay never reflects later quantity changes.
CREATE TABLE binder_addition_receipts (
    request_id TEXT PRIMARY KEY NOT NULL,
    entry_id TEXT NOT NULL,
    owner_id TEXT NOT NULL,
    card_id TEXT NOT NULL,
    language TEXT NOT NULL,
    quantity INTEGER NOT NULL,
    note TEXT,
    result TEXT NOT NULL CHECK (json_valid(result))
);
