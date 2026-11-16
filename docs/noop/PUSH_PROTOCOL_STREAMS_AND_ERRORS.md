# Push protocol: streams, errors and versioning

## Version 1 stream registry

The v1 registry is deliberately finite. A table present in NOOP's database is **not** implicitly part
of the protocol.

### Append streams

| `stream` | Natural key | `data` members |
|---|---|---|
| `hrSample` | `ts` | `bpm` |
| `rrInterval` | `ts`, `rrMs`, `seq` | `ord` (nullable), `srcChannel` (nullable), `tsSuspect` (nullable) |
| `event` | `ts`, `kind` | `payloadJSON` |
| `battery` | `ts` | `soc` (nullable), `mv` (nullable), `charging` (nullable) |
| `spo2Sample` | `ts` | `red`, `ir` |
| `skinTempSample` | `ts` | `raw`, `aux1Raw` (nullable), `aux2Raw` (nullable) |
| `respSample` | `ts` | `raw` |
| `gravitySample` | `ts` | `x`, `y`, `z`, `dynAccel` (nullable) |

`ts`, `rrMs`, `seq`, `bpm`, `red`, `ir`, and `raw` are integers. `soc`, `x`, `y`, `z`, and
`dynAccel` are finite numbers. The nullable RR metadata is integer-valued. `charging` is boolean or
null. The database's `deviceId` is supplied by the header and `synced` is intentionally omitted.

### Mutable replace-window streams

| `stream` | Key | Window selector | `data` members |
|---|---|---|---|
| `dailyMetric` | `day` | `day` | `totalSleepMin`, `efficiency`, `deepMin`, `remMin`, `lightMin`, `disturbances`, `restingHr`, `avgHrv`, `recovery`, `strain`, `exerciseCount`, `spo2Pct`, `skinTempDevC`, `respRateBpm`, `steps`, `activeKcalEst`, `spo2Red`, `spo2Ir` (all nullable) |
| `sleepSession` | `startTs` | `startTs` | `endTs`, `efficiency`, `restingHr`, `avgHrv`, `stagesJSON`, `userEdited`, `startTsAdjusted`, `motionJSON`, `sleepStateJSON`, `stagingSparse` |
| `workout` | `startTs`, `sport` | `startTs` | `endTs`, `source`, `durationS`, `energyKcal`, `avgHr`, `maxHr`, `strain`, `distanceM`, `zonesJSON`, `notes`, `routePolyline`, `steps` |
| `journal` | `day`, `question` | `day` | `answeredYes`, `notes`, `numericValue` |

Unless inherent above, mutable data members are nullable exactly as in the current Room schema.
`sleepSession.endTs`, `workout.endTs`, `workout.source`, and `journal.answeredYes` are required;
`sleepSession.userEdited` is a required boolean. `day` is `YYYY-MM-DD`; timestamp and count fields
are integers; metric and measurement fields are finite numbers. See [DATA_MODEL.md](DATA_MODEL.md)
and `android/app/src/main/java/com/noop/data/Entities.kt` for the local meanings and units. The wire
registry, not automatic reflection over either database, determines what is sent.

Newer tables such as `ppgHrSample`, `stepSample`, `sleepStateSample`, `metricSeries`, raw waveform /
IMU tables, and any future schema additions are not silently exported by v1. Adding a stream or an
optional `data` member requires a documented registry update and protocol minor version.

## Acceptance, errors, and retry idempotency

After atomically accepting an append batch or durably accepting a replace-window part, the receiver returns
2xx with `Content-Type: application/json` and exactly one acknowledgement object:

```json
{"protocolVersion":"1.0","batchId":"e835f32f-60e7-4c93-90a0-51eb6830119a","stream":"hrSample","deviceId":"strap-local-id","endCursor":{"rowId":48119,"keySha256":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"},"acceptedRows":2,"status":"accepted"}
```

Acceptance is valid only if all of these exactly match the request:

- `protocolVersion`
- `batchId`
- `stream`
- `deviceId`
- `endCursor` (including `null` for replace-window parts)
- `acceptedRows == recordCount`
- `status == "accepted"`

Any missing or mismatched member is a failed delivery even when the HTTP status is 2xx. There is no
partial success. The acknowledgement contains metadata only; it must not contain source records,
remote changes, commands, cursors chosen by the server, or configuration for NOOP to apply.
Capability metadata is confined to the separate authenticated `GET` defined above.

A receiver must remember the hash and acceptance result of each batch under
`(sourceId, deviceId, batchId)`. Repeating the same `batchId` with byte-identical **decoded NDJSON
entity bytes** returns the same acknowledgement without duplicating effects. Content coding is not
part of batch identity: gzip and identity representations of the same decoded entity are the same
batch. Reusing a `batchId` with different decoded entity bytes is a conflict and must not modify
data. Recommended failures are `400` for malformed NDJSON, `401`/`403` for auth,
`409` for conflicting identifiers or replace-window parts, `413` for a decoded body over 4 MiB (or
an encoded body over the receiver's documented wire limit), `422` for an
unsupported protocol/stream or invalid record, and `5xx` for a transient receiver failure. NOOP
automatically retries transport errors, `408`, `429`, and `5xx`. Other `4xx` responses and malformed
or mismatched acknowledgements retain progress and surface a visible configuration/protocol error;
they are retried only after a later trigger or configuration change. Responses are never consumed as
health data.

A receiver should return a bounded machine-readable body for non-2xx responses:

```json
{"type":"error","protocolVersion":"1.0","code":"registry_mismatch"}
```

`code` contains 1–64 lowercase ASCII letters, digits, or underscores and starts with a letter. Android
may display and persist only this validated code alongside the HTTP status; it never retains arbitrary
response text. Unknown error members are ignored.

The Android client reports a safe, structured cause for self-hosting diagnostics: DNS resolution,
TLS certificate or handshake, connection timeout/refused/unreachable/reset, numeric HTTP status,
invalid capabilities or acknowledgement, local encoding limits, or local database state. These
categories survive bounded continuation work so the final status keeps the original cause. Raw
exception messages and response bodies are deliberately neither displayed nor persisted because
network stacks and receiver errors can contain endpoint details, credentials, or health data.

## Versioning and forward compatibility

`protocolVersion` is `MAJOR.MINOR`. Capability discovery negotiates one exact version before Room is
opened; senders never optimistically emit a version the receiver did not select.

- A major version changes framing, required members, keys, or existing semantics. A receiver must
  reject an unsupported major version.
- A minor version may add an optional header/data member or a registry stream. Receivers supporting
  the same major must ignore unknown object members. They may reject an unknown stream without
  rejecting batches for supported streams.
- A sender must not emit a new stream or field while claiming an older minor version. Removing or
  changing the meaning/type of a field, or changing a stream key or delivery mode, requires a new
  major version.
- Receivers must reject malformed known members rather than guessing. They should preserve unknown
  `data` members if their storage model permits, but must not assign semantics to them.

Capability discovery only selects an offered version and narrows the sender's compiled registry for it.
It is not general negotiation: the receiver cannot add schemas, change delivery modes, select an
endpoint, request diagnostics, set cadence, or otherwise control NOOP.

## Apple compatibility

The contract is platform-neutral. NOOP on iOS/macOS uses GRDB/SQLite with the same natural keys and
logical v1 streams, but every implementation uses explicit registry projections rather than reflection
or `SELECT *`. A platform lacking a nullable exported column emits `null`; platform-only columns stay
absent until a later negotiated registry version. Scheduling and credential storage are platform
concerns and do not change NDJSON, acknowledgement, idempotency, or replacement semantics.
