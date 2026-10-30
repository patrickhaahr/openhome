# API guidance

- Keep HTTP handlers thin and domain logic in `src/services/`.
- For training reads, read `../docs/training-context.md`; the API owns dates, units, source resolution, freshness and coverage.
- For NOOP ingestion, read `../.noop/PUSH_PROTOCOL.md` and `../.noop/DATA_MODEL.md`. Keep its database, migrations and bearer token separate from the main API's.
- Use SQLx compile-time query macros for the main database and runtime queries for NOOP: compile-time macros only check `DATABASE_URL`. Refresh `api/.sqlx` with `just api-sqlx-prepare` after changing checked queries.
- The Food database (`food.db`, ADR 0013) is rebuildable reference data imported from the pinned DTU workbook in `services/nutrition/food_database`; it has its own `food_migrations/` and runtime queries, and nothing the user creates belongs in it. Name nutrient values by the keys and units in `services/nutrition/parameters.rs`; null is unknown, never zero.
- Meal Logs (`services/nutrition/meal_logs`) live in `app.db`: each Portion copies every known per-100 g value with its source (`food`, `label`, `estimate`) when it is logged (ADR 0012), so past Meal Logs and Nutrition Days never read the live Food database. A Meal Log's day is the Copenhagen calendar day of its time (`training::calendar`).
- Keep applied migrations unchanged; add migrations to the appropriate database's migration directory.
- For PATCH requests, absent fields preserve values and explicit `null` clears nullable fields. Map unique violations to conflicts.
- When backing up or restoring NOOP, preserve the whole database, including receiver state, health rows, batch acknowledgements and staged replacements; use a stopped API or SQLite's online backup mechanism.
