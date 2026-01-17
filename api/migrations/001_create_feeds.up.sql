CREATE TABLE feeds (
    id INTEGER PRIMARY KEY,
    url TEXT UNIQUE NOT NULL,
    enabled BOOLEAN DEFAULT TRUE
);

CREATE TABLE feed_items (
    id INTEGER PRIMARY KEY,
    feed_id INTEGER NOT NULL REFERENCES feeds(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    description TEXT,
    link TEXT NOT NULL,
    guid TEXT NOT NULL,
    pub_date DATETIME,
    UNIQUE(feed_id, guid)
);
