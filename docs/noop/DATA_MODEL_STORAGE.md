# NOOP data model: storage and migrations

## Where the database lives

The persistence layer is the `WhoopStore` Swift package
(`Packages/WhoopStore`), built on [GRDB](https://github.com/groue/GRDB.swift) over SQLite. Like
every package in the repo, it declares both platforms — `.iOS(.v16)` and `.macOS(.v13)`
(`Packages/WhoopStore/Package.swift`) — and is UI-framework agnostic, so the same schema and
storage code back both the macOS app and the iOS app (the latter build-from-source only — see
`docs/IOS.md`).

The macOS app target opens the database at a fixed, per-user location
(`Strand/Collect/StorePaths.swift`):

```
<Application Support>/OpenWhoop/whoop.sqlite
```

```swift
// Strand/Collect/StorePaths.swift
static func defaultDatabasePath() throws -> String {
    let fm = FileManager.default
    let base = try fm.url(for: .applicationSupportDirectory, in: .userDomainMask,
                          appropriateFor: nil, create: true)
        .appendingPathComponent("OpenWhoop", isDirectory: true)
    try fm.createDirectory(at: base, withIntermediateDirectories: true)
    return base.appendingPathComponent("whoop.sqlite").path
}
```

On a typical macOS install that resolves to
`~/Library/Application Support/OpenWhoop/whoop.sqlite`. Tests use an in-memory database via
`WhoopStore.inMemory()`.

### Connection configuration

`WhoopStore.init(path:)` (`Packages/WhoopStore/Sources/WhoopStore/WhoopStore.swift`) opens a
single `DatabaseQueue` and applies these PRAGMAs before any query runs:

| PRAGMA | Value | Why |
| --- | --- | --- |
| `journal_mode` | `WAL` | Two handles to the same file (the BLE collector and the metrics repository) can read/write without deadlocking. |
| `synchronous` | `NORMAL` | Durable pairing with WAL — only an OS crash or power loss can lose the last transaction. |
| `cache_size` | `-16000` | ~16 MB page cache for multi-thousand-row import/backfill writes. |
| `mmap_size` | `268435456` | 256 MB memory-mapped I/O. |
| `temp_store` | `MEMORY` | In-memory temp tables. |
| `busyMode` | `.timeout(5)` | 5-second busy timeout under write contention. |

`WhoopStore` is an `actor`: all GRDB calls run on the actor's serial executor (off the main
thread) through the `syncRead` / `syncWrite` helpers. `WhoopStoreInfo.schemaVersion` is a
separate, manually-maintained constant (currently `18`) that has lagged the real migration
history for a while and should not be read as the schema's true version. The migrator itself
(`makeMigrator()`, below) is the source of truth for what tables/columns exist, and has run
through **v25** (`v25-oura-raw` — the Oura raw-payload archive, the newest addition; see
below).

---

## Schema at a glance

The schema falls into five groups (this section predates, and undercounts, everything added
after v9 — see the schema-version note above; the Oura raw archive below is the one
post-v9 addition currently documented here):

| Group | Tables | Origin |
| --- | --- | --- |
| **Device registry** | `device` | BLE pairing |
| **Decoded streams** (durable) | `hrSample`, `rrInterval`, `event`, `battery`, `spo2Sample`, `skinTempSample`, `respSample`, `gravitySample` | Decoded from strap frames on-device |
| **Raw outbox** (transient) | `rawBatch` | Compressed raw BLE frames, prunable |
| **Bookkeeping** | `cursors` | Highwater / read cursors |
| **Metric caches** | `sleepSession`, `dailyMetric`, `journal`, `workout`, `appleDaily`, `metricSeries`, `scoreInputProvenance` | Derived metrics + their input-provider provenance + CSV / Apple-Health imports |
| **Oura raw archive** (durable, v25) | `ouraRaw` | Verbatim Oura API payloads behind the opt-in cloud import — see below |

All timestamp columns named `ts`, `startTs`, `endTs`, `capturedAt`, etc. are **unix seconds**
(integers). Day-keyed cache tables use a `day` text column in `YYYY-MM-DD` form and compare it
lexicographically.

---

## Migration history

Migrations are registered in `Packages/WhoopStore/Sources/WhoopStore/Database.swift`
(`makeMigrator()`) and run in order on every open.

| Version | What it adds |
| --- | --- |
| **v1** | Core tables: `device`, the four original decoded streams (`hrSample`, `rrInterval`, `event`, `battery`), and the raw outbox `rawBatch`. |
| **v2** | `cursors` key/value table for highwater bookkeeping. |
| **v3** | Type-47 biometric streams: `spo2Sample`, `skinTempSample`, `respSample`, `gravitySample`. |
| **v4** | Local metric caches: `sleepSession` (one row per session) and `dailyMetric` (one row per calendar day). |
| **v5** | Adds a `synced` integer column (default `0`) to all eight decoded-stream tables. **Vestigial** — see below. |
| **v6** | Adds nullable `charging` boolean to `battery` for the dense BATTERY_LEVEL series. |
| **v7** | Adds in-sleep signal aggregates to `dailyMetric`: `spo2Pct`, `skinTempDevC`, `respRateBpm` (all nullable). |
| **v8** | Adds `journal`, `workout`, and `appleDaily` (Apple-Health daily aggregates). |
| **v9** | Adds the generic long-format `metricSeries` table and its `(deviceId, key, day)` index. |
| **v24-rr-seq** | Rebuilds the `rrInterval` primary key as `(deviceId, ts, rrMs, seq)`, so an identical interval recurring within one second is no longer dropped. |
| **v29-score-input-provenance** | Adds metric-level `scoreInputProvenance` for NOOP-computed headline scores. It does not change `dayOwnership` or score precedence. |
| **v30-rr-ord** | Adds the nullable `rrInterval.ord` column — emission order within a `ts` — and makes it lead the read sort (#823/#830). Additive; pre-existing rows keep `ord` NULL. |

> This table is a selection, not the full list — it covers the migrations the tables above refer to.
> The registered set is the authority. Migrations are keyed by their **identifier string**, not by
> the number in it, so renaming one re-runs it against an already-migrated database. Cite the
> identifier in full — `v30-rr-ord`, not "v30".

### The vestigial `synced` column

Migration v5 added a per-row `synced` integer (`NOT NULL DEFAULT 0`) to each of the eight
decoded-stream tables. It dates from a since-removed server-upload feature. **NOOP is fully
offline: nothing writes or reads `synced`.** The insert path explicitly never sets it
(`StreamStore.swift`), and no read query references it. The column is left in place only to avoid
a `DROP COLUMN` migration over potentially millions of existing rows. Treat it as dead schema.

---
