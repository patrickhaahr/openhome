# Push protocol: request and delivery

## NDJSON request

Every line is one complete JSON object followed by LF (`0x0a`). There is no BOM, blank line, JSON
array, or trailing material. The first line is the batch header; exactly `recordCount` record lines
follow it.

### Header

An append batch has this shape (spacing is illustrative, not canonical):

```json
{"type":"batch","protocolVersion":"1.0","batchId":"e835f32f-60e7-4c93-90a0-51eb6830119a","sourceId":"3a3486dd-5030-4e17-a00d-a781399890f9","deviceId":"strap-local-id","stream":"hrSample","delivery":"append","recordCount":2,"startCursor":{"rowId":48110,"keySha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"},"endCursor":{"rowId":48119,"keySha256":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"}}
{"type":"record","key":{"ts":1723939201},"data":{"bpm":61}}
{"type":"record","key":{"ts":1723939202},"data":{"bpm":62}}
```

Header members are:

| Member | Meaning |
|---|---|
| `type` | Always `"batch"`. |
| `protocolVersion` | `"1.0"` for this contract. |
| `batchId` | UUID identifying these exact decoded NDJSON entity bytes. Stable across retries and content codings. |
| `sourceId` | Locally generated installation UUID. |
| `deviceId` | Local device identifier; scopes every record in the batch. |
| `stream` | A name in the versioned stream registry below. |
| `delivery` | `"append"` or `"replace_window"`, as fixed by the registry. |
| `recordCount` | Number of record lines, `0...5000`. |
| `startCursor` | Exclusive append insertion highwater, or `null` for the first append batch and replace-window parts. |
| `endCursor` | Inclusive insertion highwater of the final append record, or `null` for replace-window parts. |
| `window` | Required only for `replace_window` delivery; absent for append delivery. |

All UUIDs are lowercase canonical UUID strings. JSON object member order is not semantically
significant, but the sender must retain the exact decoded NDJSON entity bytes until that batch is
acknowledged.

### Records

Each subsequent object has `type: "record"`, a `key` containing every primary-key column other than
the header's `deviceId`, and a `data` object containing the exported non-key columns. Local-only
bookkeeping such as the vestigial `synced` column is never exported. SQL `NULL` is JSON `null`, integer
and real values are JSON numbers, text is a JSON string, and database booleans are JSON booleans.
Text columns whose names end in `JSON` remain strings; their contents are not promoted into nested
wire objects.

Keys must contain exactly the registry columns. Append records are ordered by their local insertion
position; replace-window records are sorted by natural key. Natural-key integer components compare
numerically and text components use unsigned UTF-8 byte order (SQLite `BINARY` order). A receiver
must reject duplicate keys within a batch or complete replacement window.

## Append delivery and cursors

Each append highwater is local state scoped by `(sourceId, deviceId, stream)` inside the destination
namespace above. It is an opaque receiver value carrying the sender's persistent monotonic insertion
position, not a measurement timestamp or natural primary key. The v1 member is named `rowId` because
NOOP's Android and Apple SQLite stores both map it to SQLite insertion `rowid`; a conforming non-SQLite
sender may supply an equivalent durable insertion sequence. Receivers validate and echo it but must not
interpret it as receiver state.
The sender selects rows for that device whose `rowid` is greater than `startCursor.rowId`, orders by
`rowid ASC`, and sets `endCursor.rowId` to the final row's insertion position. This matters because a
later offload can insert old measurement timestamps; a timestamp or lexicographic natural-key
highwater would permanently strand that backfill.

Each non-null cursor also contains `keySha256`, the lowercase SHA-256 of the UTF-8 bytes formed by
`stream`, LF, `deviceId`, LF, and the compact JSON natural-key object with members in registry order.
Before using a saved cursor, the sender must read
that row and verify the natural-key fingerprint. A missing row or mismatch means a restore, prune,
`VACUUM`/rowid rewrite, or database replacement invalidated the insertion positions. The sender must
reset that stream's cursor to null and replay it; receiver primary-key upserts make the replay safe.
Changing `sourceId`, normalized endpoint, selected version, or `receiverStateId` selects a fresh
progress namespace and sends a full baseline, avoiding acknowledgements being carried between
destinations. Implementations may store additional local database-generation evidence, but it is not
transmitted.

The sender also remembers, per endpoint namespace, every device scope it has considered. Live
database discovery is unioned with that encrypted set so deleting the final mutable row still emits
an empty authoritative replacement instead of leaving stale receiver data behind.

An append batch must contain at least one record. Its keys must satisfy:

```text
startCursor.rowId < first rowid < ... <= final rowid == endCursor.rowId
```

For an initial batch, `startCursor` is `null` and all insertion positions are eligible. Batches stop before either
the 5,000-record or 4-MiB decoded bound is exceeded. A receiver applies records as idempotent upserts using the
scoped primary key. Append streams do not communicate deletions.

The sender advances a stream's local highwater to `endCursor` only after a valid acceptance response
for the whole batch. A timeout, non-2xx response, invalid body, partial acceptance, or mismatched
acknowledgement leaves the highwater unchanged and retries the identical `batchId` and decoded
NDJSON entity bytes. Other
streams have independent highwaters and may continue.

## Authoritative rolling-window delivery

Mutable and recomputed tables use authoritative `replace_window` operations rather than append cursors.
After an offload, the sender evaluates the current local calendar day plus the preceding 13 local days
(approximately 14 times 24 hours across daylight-saving changes). On a fresh destination it sends that
complete 14-day baseline. Afterwards Android stores a canonical SHA-256 separately for every local day,
device, stream, source, and endpoint namespace. These hashes are local progress metadata and are never
transmitted.

If every daily hash is unchanged, no replacement request is necessary. If days changed, Android sends the
smallest contiguous window spanning those days; unchanged days between the first and last changed day may
be included. A formerly populated day whose canonical snapshot is now empty is changed and must be sent as
an empty authoritative window. Hash progress advances only after every part receives an exact durable
acknowledgement. Therefore a timeout or failed acknowledgement repeats the same deletion or replacement
instead of losing it. Receivers must accept any non-empty half-open subwindow inside the mutable horizon;
they must not require every replacement to be exactly 14 days wide.

Day-keyed windows use `YYYY-MM-DD`; timestamp-keyed windows use the corresponding local-midnight bounds
converted to Unix seconds. Bounds are always half-open:
`startInclusive <= selector < endExclusive`.

The window header member is:

```json
"window":{"replacementId":"bf8b735e-f157-4a35-beb2-9b086d10d5bd","selector":"day","startInclusive":"2026-08-05","endExclusive":"2026-08-19","part":1,"parts":1}
```

For `startTs` selectors the bounds are integer Unix seconds instead of strings. A replacement that fits
in one request uses `part: 1, parts: 1`. If it exceeds either batch bound, the sender divides the
sorted records into `parts` bounded requests. Every part has the same `replacementId`, scope, window and
positive `parts` value; `part` runs from 1 through `parts`; each part has its own stable `batchId`.
An empty window is represented by one zero-record part and is still authoritative: it deletes all
receiver rows in that scope and window. `startCursor` and `endCursor` are `null` for all parts.

Daily checksums are an upload-elision mechanism, not receiver state or a synchronization command. A
receiver remains responsible for durable storage once it acknowledges a batch. A changed destination
namespace forces the complete baseline; rotating only the bearer token preserves accepted daily hashes.

The receiver durably stages accepted parts. Only when every part is present does it atomically:

1. upsert all replacement records by the scoped primary key; and
2. delete receiver rows in the declared window whose keys are absent from the complete replacement.

An acknowledgement for the part that completes the set must not be returned until that atomic apply
succeeds. Retrying any part is harmless. Conflicting reuse of a `replacementId`, part number, or
`batchId` must be rejected. Rows outside the declared window are untouched. This absence-means-delete
rule makes edits and deletions within the rolling window converge to NOOP's local state; v1 carries no
tombstone for a row that has already aged out of that window.

A sender must have at most one incomplete replacement generation per `(sourceId, deviceId, stream)`.
Observing the first part of a different generation supersedes every older incomplete generation in that
scope, even when its exact window bounds differ. A receiver rejects late parts of a superseded
generation with `409`. Senders serialize replacement generations for a scope; retries remain safe and
parts may arrive out of numerical order.
