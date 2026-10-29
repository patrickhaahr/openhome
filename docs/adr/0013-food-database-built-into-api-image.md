# Food database built into the API image

The Danish Food Composition Database (formerly Frida) has no public API; DTU
publishes it as a versioned CC BY 4.0 workbook on DTU Data
(doi:10.11583/DTU.32312844). A Docker build stage downloads one pinned version,
verifies its checksum, and converts every food and all 232 parameters into a
read-only `food.db` with name search over the Danish and English names. The API
image ships it outside the data volume, separate from `app.db`.

## Considered Options

- Committing a generated `food.db` or seed file: puts megabytes of binary
  reference data in git for every version.
- Building it on first API start: makes startup depend on DTU's download host
  and leaves a writable copy in the data volume to drift.

## Consequences

Upgrading the database is a version and checksum change followed by a release;
no migration runs, and logged Portions keep their own values (ADR 0012). The
image build needs network access to DTU Data. Products, Meals, Supplements and
Meal Logs are household data and stay in `app.db`. Credit "© The Danish Food
Composition Database, version 6.1" wherever nutrient values are shown.
