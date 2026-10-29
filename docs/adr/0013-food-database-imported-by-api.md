# Food database imported by the API

The Danish Food Composition Database (formerly Frida) has no public API; DTU
publishes it as a versioned CC BY 4.0 workbook on DTU Data
(doi:10.11583/DTU.32312844). The Axum API owns `food.db` like `app.db` and
`noop.db`: it opens it with the shared SQLite settings and runs its own embedded
migrations. When `food.db` does not record the pinned dataset version, the API
downloads that workbook into its data directory, verifies the pinned checksum,
and imports every food and all 232 parameters with Danish and English name
search in the background. Food lookups report unavailable until the import
completes; the rest of the API starts normally.

## Considered Options

- Building `food.db` in a Docker build stage: a second path that local
  development never exercises.
- Committing a generated `food.db` or seed file: megabytes of binary reference
  data in git for every version.
- Requiring a manually supplied workbook path: a setup step in every
  environment.

## Consequences

Each dataset version needs DTU Data reachable once, on the first start after
the pin changes; the verified workbook is cached in the data directory.
Upgrading is a version and checksum change: the next start re-imports, and
logged Portions keep their own values (ADR 0012). `food.db` is rebuildable
reference data; Products, Meals, Supplements and Meal Logs stay in `app.db`.
Credit "© The Danish Food Composition Database" with its version wherever
nutrient values are shown.
