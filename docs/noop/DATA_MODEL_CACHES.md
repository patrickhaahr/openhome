# NOOP data model: bookkeeping and caches

## Bookkeeping

### `cursors` *(v2)*

A simple key/value table for incremental-processing highwater marks (`Cursors.swift`).

| Column | Type | Notes |
| --- | --- | --- |
| `name` | TEXT | **Primary key.** |
| `value` | INTEGER | Stored cursor value (typically a timestamp). |

Helpers namespace the `name`: `highwater:<stream>` (upload/forward-only highwater) and
`read:<stream>` (pull cursor). The distinct prefixes keep the two cursor families from colliding
for the same stream.

---

## Metric caches

These tables hold **derived metrics and imported aggregates** rather than raw measurements.
Recovery / strain / HRV / sleep math is computed locally by the `StrandAnalytics` package, and
CSV / Apple-Health data arrives through `StrandImport`. Every cache table follows the same
contract: a `Codable` struct, an idempotent `ON CONFLICT(...) DO UPDATE` upsert keyed by its
natural key (latest value wins), and range-read accessors that run off-main.

### `sleepSession` *(v4)*

One row per sleep session (`MetricsCache.swift`, `struct CachedSleepSession`).

| Column | Type | Notes |
| --- | --- | --- |
| `deviceId` | TEXT NOT NULL | Part of PK. |
| `startTs` | INTEGER NOT NULL | Session start, unix seconds. Part of PK. |
| `endTs` | INTEGER NOT NULL | Session end, unix seconds. |
| `efficiency` | DOUBLE | Sleep efficiency, nullable. |
| `restingHr` | INTEGER | Resting HR, nullable. |
| `avgHrv` | DOUBLE | Average HRV, nullable. |
| `stagesJSON` | TEXT | Verbatim JSON array of stage segments (`[{start,end,stage}]`), nullable — stored as a string so the cache stays schema-agnostic about staging shape. |

**Primary key:** `(deviceId, startTs)`. Read by `startTs` range, oldest first.

### `dailyMetric` *(v4, +v7 columns)*

One row per calendar day (`MetricsCache.swift`, `struct DailyMetric`). This is the central
per-day rollup behind the dashboard. **Natural key `(deviceId, day)`** where `day` is
`YYYY-MM-DD`. All metric columns are nullable.

| Column | Type | Migration | Notes |
| --- | --- | --- | --- |
| `deviceId` | TEXT NOT NULL | v4 | Part of PK. |
| `day` | TEXT NOT NULL | v4 | `YYYY-MM-DD`. Part of PK. |
| `totalSleepMin` | DOUBLE | v4 | Total sleep, minutes. |
| `efficiency` | DOUBLE | v4 | Sleep efficiency. |
| `deepMin` | DOUBLE | v4 | Deep sleep, minutes. |
| `remMin` | DOUBLE | v4 | REM sleep, minutes. |
| `lightMin` | DOUBLE | v4 | Light sleep, minutes. |
| `disturbances` | INTEGER | v4 | Disturbance count. |
| `restingHr` | INTEGER | v4 | Resting heart rate. |
| `avgHrv` | DOUBLE | v4 | Average HRV. |
| `recovery` | DOUBLE | v4 | Recovery score. |
| `strain` | DOUBLE | v4 | Day strain. |
| `exerciseCount` | INTEGER | v4 | Number of exercises. |
| `spo2Pct` | DOUBLE | v7 | Mean SpO2 (%) during sleep. |
| `skinTempDevC` | DOUBLE | v7 | Skin-temperature deviation (°C) from baseline. |
| `respRateBpm` | DOUBLE | v7 | Mean respiration rate (breaths/min) during sleep. |

**Primary key:** `(deviceId, day)`. Read by lexicographic `day` range, oldest first. The
`DailyMetric` struct's `init` defaults the three v7 fields to `nil` so older callers stay
source-compatible.

### `journal` *(v8)*

One user-answered daily prompt (`JournalWorkoutAppleCache.swift`, `struct JournalEntry`).

| Column | Type | Notes |
| --- | --- | --- |
| `deviceId` | TEXT NOT NULL | Part of PK. |
| `day` | TEXT NOT NULL | `YYYY-MM-DD`. Part of PK. |
| `question` | TEXT NOT NULL | Prompt text. Part of PK. |
| `answeredYes` | INTEGER NOT NULL | `0`/`1`, mapped to/from `Bool`. |
| `notes` | TEXT | Free-text note, nullable. |

**Primary key:** `(deviceId, day, question)`. Read by `day` range, ordered `day ASC, question
ASC`.

### `workout` *(v8)*

One workout (`JournalWorkoutAppleCache.swift`, `struct WorkoutRow`). All metric columns nullable.

| Column | Type | Notes |
| --- | --- | --- |
| `deviceId` | TEXT NOT NULL | Part of PK. |
| `startTs` | INTEGER NOT NULL | Start, unix seconds. Part of PK. |
| `endTs` | INTEGER NOT NULL | End, unix seconds. |
| `sport` | TEXT NOT NULL | Sport/activity name. Part of PK. |
| `source` | TEXT NOT NULL | Origin of the row (e.g. import source). |
| `durationS` | DOUBLE | Duration, seconds. |
| `energyKcal` | DOUBLE | Energy, kcal. |
| `avgHr` | INTEGER | Average HR. |
| `maxHr` | INTEGER | Max HR. |
| `strain` | DOUBLE | Workout strain. |
| `distanceM` | DOUBLE | Distance, meters. |
| `zonesJSON` | TEXT | Verbatim JSON of HR-zone percentages — stored as a string so the cache stays schema-agnostic about zone shape. |
| `notes` | TEXT | Free-text note. |

**Primary key:** `(deviceId, startTs, sport)`. Read by `startTs` range, oldest first.

### `appleDaily` *(v8)*

Apple-Health-specific daily aggregates (`JournalWorkoutAppleCache.swift`, `struct AppleDaily`),
imported from an Apple Health `export.xml`. All metric columns nullable.

| Column | Type | Notes |
| --- | --- | --- |
| `deviceId` | TEXT NOT NULL | Part of PK. |
| `day` | TEXT NOT NULL | `YYYY-MM-DD`. Part of PK. |
| `steps` | INTEGER | Step count. |
| `activeKcal` | DOUBLE | Active energy, kcal. |
| `basalKcal` | DOUBLE | Basal energy, kcal. |
| `vo2max` | DOUBLE | VO₂max estimate. |
| `avgHr` | INTEGER | Average HR. |
| `maxHr` | INTEGER | Max HR. |
| `walkingHr` | INTEGER | Walking HR average. |
| `weightKg` | DOUBLE | Body weight, kilograms. |

**Primary key:** `(deviceId, day)`. Read by lexicographic `day` range, oldest first.

### `metricSeries` *(v9)*

A generic **long-format / EAV** metric store (`MetricSeriesStore.swift`, `struct MetricPoint`).
Where the tables above use a wide column-per-metric layout, this is the tall counterpart: one row
per `(deviceId, day, key)` with a single REAL `value`. Any scalar metric — server-derived,
Apple-Health, journal-encoded, etc. — can be projected into this one table and read back uniformly
by key, which is the substrate for a metric explorer that lists and compares metrics without
knowing each source's schema.

| Column | Type | Notes |
| --- | --- | --- |
| `deviceId` | TEXT NOT NULL | Part of PK. |
| `day` | TEXT NOT NULL | `YYYY-MM-DD`. Part of PK. |
| `key` | TEXT NOT NULL | Metric identifier (e.g. `"restingHr"`, `"steps"`, `"recovery"`). Part of PK. |
| `value` | DOUBLE NOT NULL | The scalar value. |

**Primary key:** `(deviceId, day, key)`.

**Index** — `idx_metricSeries_device_key_day` on `(deviceId, key, day)`. The primary key orders by
`day` before `key`, so it can't efficiently serve per-metric range reads (`metricSeries(key:from:
to:)`) or `metricDays(key:)`, which scan `(deviceId, key)` and then walk days. This index makes
those reads index-only. Accessors: `upsertMetricSeries(...)`, `metricSeries(...)`,
`metricKeys(...)` (distinct keys for a device), and `metricDays(...)` (`MIN`/`MAX` day per key).

### `scoreInputProvenance` *(v29)*

Records which sensor/import source supplied the inputs for each persisted NOOP-computed score.
This is deliberately separate from `dayOwnership`, which remains a scoring resolver override.
Rows are replaced atomically with the corresponding `dailyMetric` / `metricSeries` score writes;
legacy scores without a row have unknown provenance and the UI omits their provider badge.

| Column | Type | Notes |
| --- | --- | --- |
| `deviceId` | TEXT NOT NULL | Computed `-noop` namespace. Part of PK. |
| `day` | TEXT NOT NULL | `YYYY-MM-DD`. Part of PK. |
| `key` | TEXT NOT NULL | `recovery`, `strain`, or `sleep_performance`. Part of PK. |
| `sourceId` | TEXT NOT NULL | Physical device or import source that supplied the inputs. |

**Primary key:** `(deviceId, day, key)`. **Index:** `idx_scoreInputProvenance_source` on
`sourceId`, used when a provider's data is deleted.

---

## Oura raw-payload archive

This section documents the one table added after this document's v9 baseline (see the
schema-version note above): the lossless backstop behind the opt-in Oura history import
(off by default; user-initiated OAuth backfill — `docs/PRIVACY_SECURITY.md` §1.1b). It is
**not** a metric cache like the tables above — it stores verbatim API responses, not decoded
values, so any field Oura returns can be re-derived later without re-fetching.

### `ouraRaw` *(v25)*

One row per fetched PAGE of an Oura API endpoint response — not one row per Oura document; a
single page's `data` array can carry many documents (`OuraRawStore.swift`, `struct OuraRawRow`).
Written by `OuraSyncCoordinator.fetchRaw(_:dateParam:)` in the app target (`Strand/Oura/`).
Natural key `(deviceId, endpoint, documentId)`. Migration `v25-oura-raw`
(`Packages/WhoopStore/Sources/WhoopStore/Database.swift`) — additive only, a new table, no
existing row touched.

| Column | Type | Notes |
| --- | --- | --- |
| `deviceId` | TEXT NOT NULL | Part of PK. `"oura-api"` for the live cloud-import lane. |
| `endpoint` | TEXT NOT NULL | Part of PK. Oura endpoint name, e.g. `"sleep"`, `"daily_readiness"`, `"heartrate"`. |
| `documentId` | TEXT NOT NULL | Part of PK. A SYNTHESIZED page key, `"<endpoint>-<startDate>-<pageIndex>"` (`startDate` is the backfill window's start date, `pageIndex` the fetched page's 0-based position) — never Oura's own document `id`, for any endpoint. |
| `day` | TEXT | `YYYY-MM-DD`, nullable. Currently always NULL — the coordinator never sets it. Reserved for a future per-document (rather than per-page) keying scheme. |
| `payloadJSON` | TEXT NOT NULL | Verbatim JSON body of the fetched page (the raw HTTP response, including its `data` array of documents) — losslessness holds at the page level, not the individual-document level. |
| `fetchedAt` | INTEGER NOT NULL | Unix seconds. |

**Primary key:** `(deviceId, endpoint, documentId)`. `upsertOuraRaw(...)` is idempotent on this
key via `ON CONFLICT(...) DO UPDATE` — re-pulling the SAME window (same `startDate`, so the same
`pageIndex` synthesizes the same `documentId`) overwrites that page's `day`/`payloadJSON`/
`fetchedAt` in place rather than duplicating.

**Index** — `idx_ouraRaw_device_endpoint_day` on `(deviceId, endpoint, day)`, so per-endpoint
reads (`ouraRaw(deviceId:endpoint:)`) scan `(deviceId, endpoint)` and walk `day` in order
without a table scan.

**Not covered by `deleteAllData(deviceId:)`.** Unlike the metric-cache tables above, `ouraRaw`
is not in `DeviceRegistryStore.deviceScopedTables`, so the general per-device wipe skips it by
construction. Disconnecting Oura calls the dedicated `deleteOuraRaw(deviceId:)` alongside
`deleteAllData(deviceId:)` (`Strand/Oura/OuraConnectModel.swift`) so the raw archive is purged
too, not left behind.

---

## Index summary

| Index | Table | Columns | Purpose |
| --- | --- | --- | --- |
| *(implicit PK)* | every table above | (its natural key) | Dedupe + primary lookup. |
| `idx_metricSeries_device_key_day` | `metricSeries` | `deviceId, key, day` | Index-only per-metric range reads. |
| `idx_ouraRaw_device_endpoint_day` | `ouraRaw` | `deviceId, endpoint, day` | Index-only per-endpoint range reads. |

Every other table relies on its primary-key index; the decoded-stream and date-range reads are all
served by the `(deviceId, ts)` / `(deviceId, day)` / `(deviceId, startTs)` primary keys.

---

## Provenance

NOOP's strap interoperability is built on community reverse-engineering work, which it credits and
builds upon:

- **WHOOP 4.0 protocol** — [`johnmiddleton12/my-whoop`](https://github.com/johnmiddleton12/my-whoop)
- **WHOOP 5.0 protocol** — [`b-nnett/goose`](https://github.com/b-nnett/goose)

The frame parsing, CRC, and command/event/packet decode that feed the decoded-stream tables above
live in the `WhoopProtocol` package; persistence is `WhoopStore`; the local recovery / strain /
HRV / sleep math is `StrandAnalytics`; and the CSV / Apple-Health importers are `StrandImport`.

> **Reminder.** NOOP is not affiliated with WHOOP and is not a medical device. All stored data is
> the user's own, kept entirely on the user's device.
