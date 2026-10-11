-- Accepted additions survive title edits and cascading deletion. A removed item cannot
-- be recreated by a delayed retry; a new intention needs a new request ID.
CREATE TABLE wishlist_addition_receipts (
    request_id TEXT PRIMARY KEY NOT NULL,
    item_id TEXT NOT NULL,
    title TEXT NOT NULL,
    requested_owner_id TEXT,
    requested_list_id TEXT
);
INSERT INTO wishlist_addition_receipts
    SELECT request_id, id, title, requested_owner_id, requested_list_id FROM wishlist_items;
