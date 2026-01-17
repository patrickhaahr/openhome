DROP INDEX IF EXISTS feed_items_pub_date_idx;
DROP INDEX IF EXISTS feed_items_read_at_idx;
DROP INDEX IF EXISTS feed_items_feed_id_idx;

ALTER TABLE feed_items DROP COLUMN read_at;

ALTER TABLE feeds DROP COLUMN title;
ALTER TABLE feeds DROP COLUMN etag;
ALTER TABLE feeds DROP COLUMN last_modified;
ALTER TABLE feeds DROP COLUMN last_fetched_at;
ALTER TABLE feeds DROP COLUMN last_error;
