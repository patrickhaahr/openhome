-- Meals (CONTEXT.md, Meal): named, reusable lists of items the user logs again by name. User
-- data, so they live in app.db. A Meal holds references, never values: logging it snapshots its
-- items afresh (ADR 0012), so editing or deleting a Meal never changes a Meal Log.

CREATE TABLE meals (
    id TEXT PRIMARY KEY NOT NULL,
    -- As the user named it, trimmed with inner whitespace collapsed.
    name TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 100),
    -- The lowercased name; names are unique case-insensitively, including non-ASCII letters.
    name_key TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- A Meal's Food and Product items in order: grams of a Food or of a linked Product. A Food keeps
-- the names and dataset version it was saved with, because FoodIDs change between versions; a
-- Product is read from products when the Meal is shown or logged. Supplement Doses get their
-- own table beside this one, as on Meal Logs.
CREATE TABLE meal_portions (
    meal_id TEXT NOT NULL REFERENCES meals (id) ON DELETE CASCADE,
    position INTEGER NOT NULL CHECK (position >= 0),
    grams REAL NOT NULL CHECK (grams > 0),
    food_id INTEGER CHECK (food_id > 0),
    food_name_da TEXT,
    food_name_en TEXT,
    dataset_version TEXT,
    product_barcode TEXT REFERENCES products (barcode),
    CHECK (
        (food_id IS NOT NULL AND food_name_da IS NOT NULL AND food_name_en IS NOT NULL
            AND dataset_version IS NOT NULL AND product_barcode IS NULL)
        OR (food_id IS NULL AND food_name_da IS NULL AND food_name_en IS NULL
            AND dataset_version IS NULL AND product_barcode IS NOT NULL)
    ),
    PRIMARY KEY (meal_id, position)
) WITHOUT ROWID;

-- One receipt per accepted save_meal request_id, holding its normalized arguments. No foreign
-- key: a deleted Meal keeps its receipt, so an old retry cannot recreate it.
CREATE TABLE meal_receipts (
    request_id TEXT PRIMARY KEY NOT NULL,
    meal_id TEXT NOT NULL,
    request TEXT NOT NULL
);

-- The name of the Meal a Meal Log was logged from, as it was named then; null otherwise.
ALTER TABLE meal_logs ADD COLUMN meal_name TEXT;
