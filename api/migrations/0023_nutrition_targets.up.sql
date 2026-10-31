-- Nutrition targets (CONTEXT.md, Trend Weight, Estimated Expenditure, Nutrition Goal, Calorie
-- Target, Macro Targets; ADR 0014).

-- Nutrition Goal history: each change takes effect from the Copenhagen day it is set and holds
-- until the next one. Before the first row the goal is lose 0.5 kg per week. A second change on
-- the same day replaces that day's row.
CREATE TABLE nutrition_goals (
    effective_from TEXT PRIMARY KEY NOT NULL,
    direction TEXT NOT NULL CHECK (direction IN ('lose', 'maintain', 'lean_bulk')),
    -- Kilograms per week, never signed: the direction gives the sign.
    rate_kg_per_week REAL NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    CHECK (
        (direction = 'lose' AND rate_kg_per_week IN (0.25, 0.5, 0.75))
        OR (direction = 'maintain' AND rate_kg_per_week = 0)
        OR (direction = 'lean_bulk' AND rate_kg_per_week IN (0.1, 0.25))
    )
) WITHOUT ROWID;

-- Weekly target record: the Estimated Expenditure and BMR fixed for the ISO week starting on
-- `monday`, from the Meal Logs and Body Weight dated before that Monday. Written once, on the
-- first read of any day in the week, and never updated, so later backfills and Profile changes
-- never rewrite a past week's Calorie Target. The other columns say how the expenditure was found.
CREATE TABLE nutrition_weeks (
    monday TEXT PRIMARY KEY NOT NULL,
    expenditure_kcal REAL NOT NULL CHECK (expenditure_kcal > 0),
    bmr_kcal REAL NOT NULL CHECK (bmr_kcal > 0),
    -- Mifflin-St Jeor BMR x 1.5.
    prior_kcal REAL NOT NULL CHECK (prior_kcal > 0),
    -- Null when the trailing 28 days could not give one (no logged day or no Trend Weight change).
    data_estimate_kcal REAL,
    -- Days of the trailing 28 with known logged energy, and the data estimate's blend weight.
    logged_days INTEGER NOT NULL CHECK (logged_days BETWEEN 0 AND 28),
    blend_weight REAL NOT NULL CHECK (blend_weight BETWEEN 0 AND 1),
    -- The Trend Weight the BMR was computed from, and the day it was read on.
    trend_weight_kg REAL NOT NULL CHECK (trend_weight_kg > 0),
    trend_weight_day TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) WITHOUT ROWID;
