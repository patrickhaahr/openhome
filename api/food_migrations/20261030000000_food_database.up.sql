-- The Danish Food Composition Database, imported from DTU's pinned workbook (ADR 0013).
-- Rebuildable reference data: every import replaces all rows in one transaction.

-- The one complete import the Foods below come from.
CREATE TABLE dataset (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    version TEXT NOT NULL,
    sha256 TEXT NOT NULL,
    imported_at TEXT NOT NULL
);

-- Every parameter the workbook defines, keyed by DTU ParameterID with OpenHome's vocabulary key.
CREATE TABLE parameter (
    parameter_id INTEGER PRIMARY KEY,
    key TEXT NOT NULL UNIQUE,
    name_da TEXT NOT NULL,
    name_en TEXT NOT NULL,
    group_da TEXT NOT NULL,
    group_en TEXT NOT NULL,
    -- NULL for unitless factors such as pH.
    unit TEXT,
    -- 1 when values are amounts per 100 g edible part; 0 for properties such as waste and density.
    per_100g INTEGER NOT NULL CHECK (per_100g IN (0, 1)),
    sort_key INTEGER NOT NULL
);

CREATE TABLE food (
    food_id INTEGER PRIMARY KEY,
    name_da TEXT NOT NULL,
    name_en TEXT NOT NULL,
    food_group_id INTEGER,
    food_group_da TEXT,
    food_group_en TEXT
);

-- An empty workbook cell has no row: unknown, never zero. A recorded zero is stored as 0.
CREATE TABLE food_value (
    food_id INTEGER NOT NULL REFERENCES food (food_id),
    parameter_id INTEGER NOT NULL REFERENCES parameter (parameter_id),
    value REAL NOT NULL,
    PRIMARY KEY (food_id, parameter_id)
) WITHOUT ROWID;
