-- Supplements (CONTEXT.md, Supplement): labelled products whose amounts are given per dose
-- (tablet, capsule, scoop or ml) rather than per 100 g. User data, so they live in app.db.

CREATE TABLE supplements (
    id TEXT PRIMARY KEY NOT NULL,
    -- As the user named it, trimmed with inner whitespace collapsed.
    name TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 100),
    -- The lowercased name; names are unique case-insensitively, including non-ASCII letters.
    name_key TEXT NOT NULL UNIQUE,
    dose_unit TEXT NOT NULL CHECK (dose_unit IN ('tablet', 'capsule', 'scoop', 'ml')),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- One dose's amount of a Food database parameter, keyed and united as in the parameter
-- vocabulary. form names the chemical form where Upper Limits differ by form (folic acid,
-- retinol, nicotinamide), so a supplement-only Upper Limit can find its share; null otherwise.
CREATE TABLE supplement_amounts (
    supplement_id TEXT NOT NULL REFERENCES supplements (id) ON DELETE CASCADE,
    position INTEGER NOT NULL CHECK (position >= 0),
    key TEXT NOT NULL,
    form TEXT,
    value REAL NOT NULL CHECK (value >= 0),
    unit TEXT NOT NULL,
    PRIMARY KEY (supplement_id, position)
) WITHOUT ROWID;

-- One dose's amount of a named compound the Food database has no parameter for (creatine).
CREATE TABLE supplement_compounds (
    supplement_id TEXT NOT NULL REFERENCES supplements (id) ON DELETE CASCADE,
    position INTEGER NOT NULL CHECK (position >= 0),
    name TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 100),
    value REAL NOT NULL CHECK (value >= 0),
    unit TEXT NOT NULL CHECK (unit IN ('g', 'mg', 'µg')),
    PRIMARY KEY (supplement_id, position)
) WITHOUT ROWID;

-- One receipt per accepted add_supplement request_id, holding its normalized arguments. No
-- foreign key: a deleted Supplement keeps its receipt, so an old retry cannot recreate it.
CREATE TABLE supplement_receipts (
    request_id TEXT PRIMARY KEY NOT NULL,
    supplement_id TEXT NOT NULL,
    request TEXT NOT NULL
);

-- A Meal's Supplement Doses in order, beside its Portions. A Meal holds references: a dose is
-- snapshotted when the Meal is logged. A Supplement in a Meal cannot be deleted.
CREATE TABLE meal_supplement_doses (
    meal_id TEXT NOT NULL REFERENCES meals (id) ON DELETE CASCADE,
    position INTEGER NOT NULL CHECK (position >= 0),
    supplement_id TEXT NOT NULL REFERENCES supplements (id),
    doses REAL NOT NULL CHECK (doses > 0),
    PRIMARY KEY (meal_id, position)
) WITHOUT ROWID;
CREATE INDEX meal_supplement_doses_supplement ON meal_supplement_doses (supplement_id);

-- A Meal Log's Supplement Doses, beside its Portions: the Supplement as it was named and
-- labelled when logged (ADR 0012). No foreign key to supplements: a later edit or deletion never
-- changes a Meal Log.
CREATE TABLE meal_log_supplement_doses (
    id INTEGER PRIMARY KEY,
    meal_log_id TEXT NOT NULL REFERENCES meal_logs (id) ON DELETE CASCADE,
    position INTEGER NOT NULL CHECK (position >= 0),
    supplement_id TEXT NOT NULL,
    name TEXT NOT NULL,
    dose_unit TEXT NOT NULL CHECK (dose_unit IN ('tablet', 'capsule', 'scoop', 'ml')),
    doses REAL NOT NULL CHECK (doses > 0),
    UNIQUE (meal_log_id, position)
);

-- A logged dose's snapshot of its per-dose parameter amounts, as in supplement_amounts.
CREATE TABLE meal_log_dose_amounts (
    dose_id INTEGER NOT NULL REFERENCES meal_log_supplement_doses (id) ON DELETE CASCADE,
    position INTEGER NOT NULL CHECK (position >= 0),
    key TEXT NOT NULL,
    form TEXT,
    value REAL NOT NULL CHECK (value >= 0),
    unit TEXT NOT NULL,
    PRIMARY KEY (dose_id, position)
) WITHOUT ROWID;

-- A logged dose's snapshot of its per-dose named compounds, as in supplement_compounds.
CREATE TABLE meal_log_dose_compounds (
    dose_id INTEGER NOT NULL REFERENCES meal_log_supplement_doses (id) ON DELETE CASCADE,
    position INTEGER NOT NULL CHECK (position >= 0),
    name TEXT NOT NULL,
    value REAL NOT NULL CHECK (value >= 0),
    unit TEXT NOT NULL,
    PRIMARY KEY (dose_id, position)
) WITHOUT ROWID;
