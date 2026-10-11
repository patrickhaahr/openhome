CREATE TABLE household_users (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL COLLATE NOCASE UNIQUE CHECK (length(trim(name)) BETWEEN 1 AND 200)
);

CREATE TABLE wishlists (
    id TEXT PRIMARY KEY NOT NULL,
    owner_id TEXT NOT NULL REFERENCES household_users(id) ON DELETE CASCADE,
    name TEXT NOT NULL COLLATE NOCASE CHECK (length(trim(name)) BETWEEN 1 AND 200),
    UNIQUE (owner_id, name)
);

CREATE TABLE wishlist_items (
    id TEXT PRIMARY KEY NOT NULL,
    list_id TEXT NOT NULL REFERENCES wishlists(id) ON DELETE CASCADE,
    title TEXT NOT NULL CHECK (length(trim(title)) BETWEEN 1 AND 500),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    request_id TEXT NOT NULL UNIQUE,
    requested_owner_id TEXT,
    requested_list_id TEXT
);
CREATE INDEX wishlist_items_list ON wishlist_items(list_id, created_at, id);
