# 03: Replace-window delivery for the 4 mutable streams

**What to build:** The four mutable streams (`dailyMetric`, `sleepSession`, `workout`, `journal`) work with `replace_window` delivery, completing the registry so the client can baseline all 12 streams. Durable staging of accepted parts keyed by `replacementId`/`part`; atomic apply once all parts are present: upsert the replacement records by scoped PK, then delete rows in the declared window whose keys are absent. Empty windows (a zero-record part) delete all rows in scope and window. Multi-part windows share `replacementId`, window, and `parts`; parts may arrive out of order. The first part of a new generation supersedes older incomplete generations in the same `(sourceId, deviceId, stream)` scope; late parts of a superseded generation → 409. Day-keyed selectors use `YYYY-MM-DD` bounds; timestamp-keyed use half-open integer Unix seconds. Rows outside the window are untouched. Ack carries `endCursor: null`. All 12 stream names are advertised in capabilities.

**Blocked by:** 02 (ingest pipeline, ledger, registry validation).

**Status:** ready-for-agent

**Reference (read in this order):** `.noop/PUSH_PROTOCOL.md` — "Authoritative rolling-window delivery" + "Version 1 stream registry" (mutable tables) + "Acceptance, errors" are the spec for this ticket. Then `.noop/PushModels.kt` and the `window` member section of `PushProtocol.kt` for exact field shapes. Only open `.noop/PushCoordinator.kt` or `.noop/tests/` to resolve a specific ambiguity (e.g. generation supersede or empty-window semantics).

- [ ] A complete replacement for each mutable stream upserts records and deletes absent keys in the window; rows outside the window survive
- [ ] An empty window deletes all rows for that scope and window
- [ ] Parts arrive out of order; ack for the final part is returned only after the atomic apply commits
- [ ] Conflicting reuse of `replacementId`, part number, or `batchId` → 409, no data change
- [ ] A first part of a new generation supersedes an older incomplete one; late parts of the superseded generation → 409
- [ ] Byte-identical part retries replay their ack; window bounds validated as half-open, day format and Unix-second bounds enforced
- [ ] Capabilities now list all 12 streams; required fields (`endTs`, `source`, `userEdited`, `answeredYes`) enforced per registry
