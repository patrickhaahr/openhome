# 02: Append batch ingest for all 8 append streams

**What to build:** The NOOP client can push append batches for all eight append streams (`hrSample`, `rrInterval`, `event`, `battery`, `spo2Sample`, `skinTempSample`, `respSample`, `gravitySample`) and they land in columnar SQLite tables. The full POST pipeline works end to end: bearer auth, gzip decode, 4 MiB decoded-NDJSON bound, NDJSON framing (header line + exactly `recordCount` record lines), header validation, per-stream record validation against the v1 registry (key columns exactly, typed data members, duplicate keys rejected), and idempotent upserts keyed by `(sourceId, deviceId, stream, PK)`. Cursor validation enforces `startCursor.rowId < first < ... <= endCursor.rowId`. The batch ledger replays the same ack for byte-identical retries regardless of Content-Encoding, returns 409 for conflicting reuse of a `batchId`, and every failure returns a bounded `{"type":"error","protocolVersion":"1.0","code":"..."}` body. The ack exactly matches the contract: echoing `protocolVersion`, `batchId`, `stream`, `deviceId`, `endCursor`, `acceptedRows == recordCount`, `status: "accepted"`.

**Blocked by:** 01 (capabilities endpoint, config).

**Status:** ready-for-agent

**Reference (read in this order):** `.noop/PUSH_PROTOCOL.md` — the wire contract; the whole doc is the spec for this ticket (framing, cursors, ack, idempotency, error codes). Then `.noop/PushModels.kt` for exact field names/types. Only open `.noop/PushProtocol.kt`, `PushDao.kt`, `PushCoordinator.kt`, or `.noop/tests/` to resolve a specific ambiguity (e.g. how cursors or byte-identity are computed). `.noop/DATA_MODEL.md` is optional context; `.noop/SCOPE.md` skip it.

- [x] Valid append batch for each of the 8 streams is stored columnar (one typed table per stream) and acknowledged with an exact-match ack
- [x] Byte-identical retry of a batch replays the stored ack without re-applying rows, across gzip and identity encodings
- [x] Reused `batchId` with different decoded bytes → 409, no data change
- [x] Malformed NDJSON → 400; unsupported stream/protocol or invalid record → 422; decoded body over 4 MiB → 413; all with bounded error codes
- [x] Upsert with the same scoped PK updates rather than duplicates; two `sourceId`s never overwrite each other
- [x] Append batches with 0 records rejected; cursors validated per the contract inequality
- [x] `keySha256` values are accepted and echoed verbatim, never interpreted
