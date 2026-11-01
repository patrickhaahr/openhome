-- Products (CONTEXT.md, Product): packaged foods by barcode, each linked to the one Food that fills
-- the values its label lacks. User data, so it lives in app.db; FoodIDs refer to food.db, whose
-- names are kept beside the ID because a dataset upgrade can replace them.
CREATE TABLE products (
    -- Canonical digits: EAN-8 or GTIN-13.
    barcode TEXT PRIMARY KEY NOT NULL
        CHECK (length(barcode) IN (8, 13) AND barcode NOT GLOB '*[^0-9]*'),
    name TEXT,
    brand TEXT,
    -- Open Food Facts' nova_group.
    nova_class INTEGER CHECK (nova_class BETWEEN 1 AND 4),
    food_id INTEGER NOT NULL,
    food_name_da TEXT NOT NULL,
    food_name_en TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- The label's values per 100 g, keyed and united as in the parameter vocabulary. A value the
-- label lacks has no row: unknown, never zero.
CREATE TABLE product_label_values (
    barcode TEXT NOT NULL REFERENCES products (barcode) ON DELETE CASCADE,
    key TEXT NOT NULL,
    value REAL NOT NULL,
    unit TEXT NOT NULL,
    PRIMARY KEY (barcode, key)
) WITHOUT ROWID;

-- One receipt per accepted link_product request_id.
CREATE TABLE product_link_receipts (
    request_id TEXT PRIMARY KEY NOT NULL,
    barcode TEXT NOT NULL,
    food_id INTEGER NOT NULL
);

-- A Product Portion keeps the Product it was logged as; food_id stays the Food linked then.
ALTER TABLE meal_log_portions ADD COLUMN product_barcode TEXT;
ALTER TABLE meal_log_portions ADD COLUMN product_name TEXT;
ALTER TABLE meal_log_portions ADD COLUMN product_brand TEXT;
