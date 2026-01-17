ALTER TABLE feeds ADD COLUMN title TEXT;
ALTER TABLE feeds ADD COLUMN etag TEXT;
ALTER TABLE feeds ADD COLUMN last_modified TEXT;
ALTER TABLE feeds ADD COLUMN last_fetched_at DATETIME;
ALTER TABLE feeds ADD COLUMN last_error TEXT;

ALTER TABLE feed_items ADD COLUMN read_at DATETIME;

CREATE INDEX feed_items_pub_date_idx ON feed_items(pub_date);
CREATE INDEX feed_items_read_at_idx ON feed_items(read_at);
CREATE INDEX feed_items_feed_id_idx ON feed_items(feed_id);
