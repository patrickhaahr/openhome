-- Meal Logs (CONTEXT.md, Nutrition): what the user ate, at an explicit time. User data, so it
-- lives in app.db; Foods stay in the rebuildable food.db. Every Portion keeps its own copy of the
-- per-100 g values it was logged with (ADR 0012), so a later import never rewrites the past.

CREATE TABLE meal_logs (
    id TEXT PRIMARY KEY NOT NULL,
    -- The instant eaten, in UTC: 'YYYY-MM-DDTHH:MM:SSZ'.
    eaten_at TEXT NOT NULL,
    -- The Europe/Copenhagen calendar day of eaten_at, midnight to midnight.
    day TEXT NOT NULL CHECK (day GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'),
    note TEXT CHECK (note IS NULL OR length(trim(note)) BETWEEN 1 AND 500),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE INDEX meal_logs_day ON meal_logs (day, eaten_at);

-- A Portion: grams of a Food, in the Meal Log's order. FoodIDs change between dataset versions,
-- so the Food's names and the dataset version are kept beside its ID.
CREATE TABLE meal_log_portions (
    id INTEGER PRIMARY KEY,
    meal_log_id TEXT NOT NULL REFERENCES meal_logs (id) ON DELETE CASCADE,
    position INTEGER NOT NULL CHECK (position >= 0),
    grams REAL NOT NULL CHECK (grams > 0),
    food_id INTEGER NOT NULL,
    name_da TEXT NOT NULL,
    name_en TEXT NOT NULL,
    dataset_version TEXT NOT NULL,
    UNIQUE (meal_log_id, position)
);

-- A Portion's snapshot: every per-100 g value, keyed and united as in the parameter vocabulary,
-- with where it came from (the food database, a Product label or an agent estimate). An unknown
-- value has no row: unknown, never zero.
CREATE TABLE meal_log_portion_values (
    portion_id INTEGER NOT NULL REFERENCES meal_log_portions (id) ON DELETE CASCADE,
    key TEXT NOT NULL,
    value REAL NOT NULL,
    unit TEXT NOT NULL,
    source TEXT NOT NULL CHECK (source IN ('food', 'label', 'estimate')),
    PRIMARY KEY (portion_id, key)
) WITHOUT ROWID;

-- One receipt per accepted log_meal request_id, holding its normalized arguments. No foreign key:
-- a deleted Meal Log keeps its receipt, so an old retry cannot recreate it.
CREATE TABLE meal_log_receipts (
    request_id TEXT PRIMARY KEY NOT NULL,
    meal_log_id TEXT NOT NULL,
    request TEXT NOT NULL
);
