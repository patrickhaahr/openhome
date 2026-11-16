# NOOP data model: devices, streams and outbox

## Device registry

### `device` *(v1)*

One row per strap the app has seen. Natural key is the device `id`.

| Column | Type | Notes |
| --- | --- | --- |
| `id` | TEXT | **Primary key.** Stable device identifier. |
| `mac` | TEXT | BLE MAC address (nullable). |
| `name` | TEXT | Advertised/device name (nullable). |
| `firstSeen` | INTEGER | Unix seconds, set on first insert. |
| `lastSeen` | INTEGER | Unix seconds, bumped on every upsert. |

`upsertDevice(id:mac:name:)` (`StreamStore.swift`) inserts or, on `id` conflict, updates `mac`,
`name`, and `lastSeen` (it does not touch `firstSeen`). Every other table references the strap via
a `deviceId` text column, scoping all data per device.

---

## Decoded streams (durable record)

These eight tables are the **durable, compact local record** of what the strap measured. They are
decoded on-device from BLE frames by the `WhoopProtocol` package and written by
`WhoopStore.insert(_ streams:deviceId:)` (`StreamStore.swift`). The in-memory shapes are the
`WhoopProtocol` stream structs (`Packages/WhoopProtocol/Sources/WhoopProtocol/Streams.swift`):
`HRSample`, `RRInterval`, `WhoopEvent`, `BatterySample`, `SpO2Sample`, `SkinTempSample`,
`RespSample`, `GravitySample`, aggregated into `Streams`.

All inserts are **idempotent by natural key** — they use `ON CONFLICT(...) DO NOTHING`, so
re-decoding overlapping frames (the common case during BLE backfill) never duplicates rows.
`insert(...)` returns the count of rows *actually* inserted per stream. Range reads live in
`Reads.swift` and follow a uniform shape: `WHERE deviceId = ? AND ts >= ? AND ts <= ? ORDER BY ts
ASC LIMIT ?`.

> The biometric stream structs carry a constant `unit` field (`"raw_adc"` / `"g"`) for JSON
> parity with golden fixtures, but `unit` is **not** a database column — only the numeric fields
> below are persisted.

### `hrSample` *(v1)* — heart rate

| Column | Type | Notes |
| --- | --- | --- |
| `deviceId` | TEXT NOT NULL | Part of PK. |
| `ts` | INTEGER NOT NULL | Wall-clock unix seconds. Part of PK. |
| `bpm` | INTEGER NOT NULL | Beats per minute. |
| `synced` | INTEGER NOT NULL DEFAULT 0 | *(v5, vestigial)* |

**Primary key:** `(deviceId, ts)`. HR is taken only from `REALTIME_DATA` (type 40) frames.
`latestHRSampleTs(deviceId:)` returns `MAX(ts)` here — the biometric "data frontier" used by the
stuck-strap watchdog.

### `rrInterval` *(v1)* — R-R intervals (HRV source)

| Column | Type | Notes |
| --- | --- | --- |
| `deviceId` | TEXT NOT NULL | Part of PK. |
| `ts` | INTEGER NOT NULL | Wall-clock unix seconds. Part of PK. |
| `rrMs` | INTEGER NOT NULL | Beat-to-beat interval, milliseconds. Part of PK. |
| `seq` | INTEGER NOT NULL DEFAULT 0 | *(v24)* Repeat counter for an **identical** beat. Part of PK. |
| `ord` | INTEGER | *(v30)* Emission order within `ts`. Nullable; **not** in the PK. |
| `synced` | INTEGER NOT NULL DEFAULT 0 | *(v5, vestigial)* |

**Primary key:** `(deviceId, ts, rrMs, seq)` — `rrMs` is in the key because multiple R-R intervals can
share a single `REALTIME_DATA` timestamp, and `seq` because the same interval value can legitimately
recur within that second. `seq` keys on `(ts, rrMs)`, **not** `ts` alone: every *distinct* interval in
a second therefore carries `seq = 0` and keeps its own key, so a distinct beat is never dropped when a
second arrives across separate insert batches or via the live/historical merge. A `ts`-only counter
would restart per batch and collide distinct beats — a data-loss regression.

**Read order:** `ts ASC, ord ASC, rrMs ASC, seq ASC`
(`Reads.swift` `rrIntervals`, `WhoopDao.kt` `rrIntervals`).

`ord` leads the sort because ordering by `rrMs` returned a second's beats sorted by **value**, which
makes successive beats similar by construction and biases RMSSD — built entirely from successive
differences — downward (#823, fixed in #830). `ord` is the beat's position among all beats sharing its
`ts`, stamped at decode time. It is deliberately **not** in the key, for the reason above.

Two properties of `ord` a consumer has to know:

- **Pre-v30 rows have `ord` NULL.** The order was never recorded and cannot be backfilled. SQLite
  sorts NULL first in ASC, so an all-legacy second ties on `ord` and falls through to the old
  `(rrMs, seq)` order — i.e. existing data reads back exactly as before, with the #823 bias intact.
  No `COALESCE`, no sentinel. Room and GRDB agree here because both are SQLite.
- **`ord` is batch-local.** A second split across two live flushes restarts `ord` at 0, and
  `ON CONFLICT DO NOTHING` keeps whichever row landed first, so that second also falls back to
  magnitude order. The historical offload path delivers a second atomically and is unaffected.

`ord` is a sort key only. The two platforms differ in whether they carry it back: Swift selects
`ts, rrMs`, so `ord` is excluded, while `WhoopDao.rrIntervals` is `SELECT *` and Room materialises it
into every returned `RrInterval` (`Entities.kt`, `val ord: Int? = null`). No consumer reads its value
on either platform.

### `event` *(v1)* — strap events

| Column | Type | Notes |
| --- | --- | --- |
| `deviceId` | TEXT NOT NULL | Part of PK. |
| `ts` | INTEGER NOT NULL | Real RTC unix seconds (never offset). Part of PK. |
| `kind` | TEXT NOT NULL | Event name (e.g. `BATTERY_LEVEL(3)`). Part of PK. |
| `payloadJSON` | TEXT NOT NULL | Decoded payload as JSON. |
| `synced` | INTEGER NOT NULL DEFAULT 0 | *(v5, vestigial)* |

**Primary key:** `(deviceId, ts, kind)`. `payloadJSON` is serialized with `JSONEncoder`'s
`.sortedKeys` so the same payload is byte-identical every time — important for the natural-key
dedupe. Reads decode it back into `[String: ParsedValue]` with a shared, reused decoder.

### `battery` *(v1, +charging in v6)*

| Column | Type | Notes |
| --- | --- | --- |
| `deviceId` | TEXT NOT NULL | Part of PK. |
| `ts` | INTEGER NOT NULL | Event RTC for BATTERY_LEVEL events, else `wallClockRef`. Part of PK. |
| `soc` | DOUBLE | State of charge (%), nullable. |
| `mv` | INTEGER | Millivolts, nullable. |
| `charging` | BOOLEAN | *(v6)* Nullable — only the dense BATTERY_LEVEL event series reports it; the command-response path leaves it `NULL`. |
| `synced` | INTEGER NOT NULL DEFAULT 0 | *(v5, vestigial)* |

**Primary key:** `(deviceId, ts)`. (Note: `batterySamples(...)` reads back only `ts, soc, mv`.)

### Type-47 biometric streams *(v3)*

These four mirror the original streams (per-row natural key `(deviceId, ts)`, `DO NOTHING`
inserts, identical range-read shape).

#### `spo2Sample` — pulse oximetry raw ADC

| Column | Type | Notes |
| --- | --- | --- |
| `deviceId` | TEXT NOT NULL | Part of PK. |
| `ts` | INTEGER NOT NULL | Unix seconds. Part of PK. |
| `red` | INTEGER NOT NULL | Red LED raw ADC. |
| `ir` | INTEGER NOT NULL | IR LED raw ADC. |
| `synced` | INTEGER NOT NULL DEFAULT 0 | *(v5, vestigial)* |

**Primary key:** `(deviceId, ts)`.

#### `skinTempSample` — skin temperature raw ADC

| Column | Type | Notes |
| --- | --- | --- |
| `deviceId` | TEXT NOT NULL | Part of PK. |
| `ts` | INTEGER NOT NULL | Unix seconds. Part of PK. |
| `raw` | INTEGER NOT NULL | Raw ADC reading. |
| `synced` | INTEGER NOT NULL DEFAULT 0 | *(v5, vestigial)* |

**Primary key:** `(deviceId, ts)`.

#### `respSample` — respiration raw ADC

| Column | Type | Notes |
| --- | --- | --- |
| `deviceId` | TEXT NOT NULL | Part of PK. |
| `ts` | INTEGER NOT NULL | Unix seconds. Part of PK. |
| `raw` | INTEGER NOT NULL | Raw ADC reading. |
| `synced` | INTEGER NOT NULL DEFAULT 0 | *(v5, vestigial)* |

**Primary key:** `(deviceId, ts)`.

#### `gravitySample` — accelerometer / gravity vector

| Column | Type | Notes |
| --- | --- | --- |
| `deviceId` | TEXT NOT NULL | Part of PK. |
| `ts` | INTEGER NOT NULL | Unix seconds. Part of PK. |
| `x` | DOUBLE NOT NULL | Gravity vector X (g). |
| `y` | DOUBLE NOT NULL | Gravity vector Y (g). |
| `z` | DOUBLE NOT NULL | Gravity vector Z (g). |
| `synced` | INTEGER NOT NULL DEFAULT 0 | *(v5, vestigial)* |

**Primary key:** `(deviceId, ts)`.

This is the low-rate decoded vector used by normal analytics, not the 100 Hz six-axis capture.
High-rate IMU deliberately remains outside SQLite. Capture sessions own canonical, decoded `.imus`
files split into fixed 30-minute UTC segments with independently compressed 30-second blocks. See
[5/MG raw data capture](RAW_DATA_CAPTURE.md#storage-design).

---

## Raw outbox (transient, prunable)

### `rawBatch` *(v1)*

The raw outbox stores the strap's original BLE frames — compressed and batched — so the exact
bytes survive even for frames NOOP can't yet fully decode. Whereas the decoded streams are durable,
raw batches are **transient and prunable**. Implementation in `RawOutbox.swift`.

| Column | Type | Notes |
| --- | --- | --- |
| `batchId` | TEXT | **Primary key.** |
| `deviceId` | TEXT NOT NULL | Owning strap. |
| `capturedAt` | INTEGER NOT NULL | Unix seconds the batch was captured; pending reads order by this. |
| `deviceClockRef` | INTEGER NOT NULL | Strap-clock reference for the wall-clock offset. |
| `wallClockRef` | INTEGER NOT NULL | Wall-clock reference, unix seconds. |
| `startTs` | INTEGER NOT NULL | First frame timestamp in the batch. |
| `endTs` | INTEGER NOT NULL | Last frame timestamp in the batch. |
| `frameCount` | INTEGER NOT NULL | Number of frames packed. |
| `byteSize` | INTEGER NOT NULL | Size used for `storageStats()` totals. |
| `framesBlob` | BLOB NOT NULL | zlib-compressed packed frames (length-prefixed). |
| `syncedAt` | INTEGER | Unix seconds; `NULL` until marked. |

**Primary key:** `batchId`. Frames are packed as `[count u32 LE]{[len u32 LE][bytes]}×count`,
then zlib-compressed with a 4-byte uncompressed-length prefix.

**Pruning policy** (`pruneRaw(now:keepWindowSeconds:maxUnsyncedBytes:)`): only batches with a
non-null `syncedAt` older than `now - keepWindowSeconds` are deleted — safe because the decoded
streams persist separately. Unsynced raw is **never** dropped (it is the sole copy of the strap's
not-yet-decoded bytes after a chunk is trimmed). `maxUnsyncedBytes` is accepted for call-site
compatibility but intentionally unused.

---
