ALTER TABLE wishlist_items ADD COLUMN url TEXT;
ALTER TABLE wishlist_items ADD COLUMN image_url TEXT;
ALTER TABLE wishlist_items ADD COLUMN notes TEXT;
ALTER TABLE wishlist_items ADD COLUMN price TEXT;
ALTER TABLE wishlist_items ADD COLUMN quantity INTEGER NOT NULL DEFAULT 1 CHECK (quantity BETWEEN 1 AND 999);
ALTER TABLE wishlist_items ADD COLUMN priority TEXT NOT NULL DEFAULT 'normal' CHECK (priority IN ('low', 'normal', 'high'));
-- Preserve title-only additions' identity using the same canonical JSON as Axum.
ALTER TABLE wishlist_addition_receipts ADD COLUMN details TEXT NOT NULL DEFAULT '{"image_url":null,"notes":null,"price":null,"priority":"normal","quantity":1,"url":null}';
