# Push protocol: transport and identity

## Transport and authentication

The configured endpoint serves authenticated capabilities on `GET` and accepts one `POST` per batch.
The settings screen may issue this `GET` alone when the user selects **Test connection**; that action
does not open the health database or send a batch.

```http
GET /the/user-configured-path HTTP/1.1
Accept: application/json
Authorization: Bearer <user-supplied-token>
NOOP-Push-Accept-Version: 1.0
```

A successful capability response has these required members:

```json
{"type":"capabilities","protocolVersion":"1.0","receiverStateId":"5fc7b9a0-8055-4e49-a308-3a290f98d81a","streams":["hrSample","rrInterval","dailyMetric"]}
```

`NOOP-Push-Accept-Version` is a comma-separated, sender-preferred list of exact versions it can emit.
The receiver selects the first version in that order it supports and returns it as `protocolVersion`; no common version fails
with `406` before health data is read. `receiverStateId` is a canonical UUID persisted with receiver
state. It remains stable across upgrades, database backups and migrations, and changes whenever the
receiver no longer has continuity with previously acknowledged data. Restoring a stale receiver backup
therefore requires the operator to rotate this ID. Rotation starts a new idempotency generation: the
receiver retains health records but atomically discards old batch acknowledgements, replacement staging,
and generation fences so deterministic baseline batch IDs are applied again rather than short-circuited.

`streams` is a duplicate-free subset of the twelve names in the selected v1 registry; array order has
no semantic meaning. An empty array is valid. Unknown names, duplicate names, a missing required member,
an unsupported version, malformed JSON, or a response over 16 KiB fail closed before Room is opened or
health data is encoded. Unknown optional object members are ignored within a supported major version. The
receiver cannot add tables or fields: the effective registry is always the intersection of its list
and the client's compiled v1 registry. Android performs no snapshot read and no batch `POST` for an
unadvertised stream.

Capability changes affect future attempts only. A client retains progress for an unadvertised
stream, so advertising it again resumes from the existing cursor. Removing a stream from the list
is not a deletion command and cannot remove records already stored by the receiver.

There is no implicit capability fallback: `404`, `405`, a missing version response, and every invalid
capability document send no health data. Transport failures, `408`, `429`, and `5xx` are retryable and
send no batch in that attempt; other failures are visible protocol/configuration errors. Redirects are
never followed and the bearer token is never forwarded.

Batch delivery then uses:

```http
POST /the/user-configured-path HTTP/1.1
Content-Type: application/x-ndjson; charset=utf-8
Accept: application/json
Authorization: Bearer <user-supplied-token>
Content-Encoding: gzip
```

- HTTPS endpoints may be public. Plain HTTP is accepted only when the URL uses a numeric loopback,
  RFC 1918 private IPv4, IPv4/IPv6 link-local, or IPv6 ULA address. Cleartext hostnames (including
  `.local`) and public cleartext destinations are rejected, eliminating a DNS-rebinding boundary. A
  bearer token sent over allowed local HTTP is still visible locally, so HTTPS remains preferable.
- The token and endpoint are supplied by the user. Neither identifies a NOOP account; no such account
  exists.
- One request contains exactly one device and one stream. A v1 request contains at most **5,000 record
  lines** and at most **4 MiB (4,194,304 bytes)** of decoded UTF-8 NDJSON, including newlines.
- Android sends request entities with `Content-Encoding: gzip` and an explicit `Content-Length`.
  The decoded NDJSON bound is authoritative; the sender also caps the encoded wire entity at
  **4 MiB + 64 KiB** so compression cannot introduce unbounded buffering. Receivers must decode
  before enforcing the NDJSON limit. Other conforming senders may use identity encoding while
  applying the same 4-MiB decoded bound.
- For compatibility with protocol-1.0 identity-only receivers, Android retries once without
  `Content-Encoding` only after a definitive HTTP `415 Unsupported Media Type`. The fallback uses
  the same endpoint, authorization, `batchId`, and byte-identical decoded NDJSON entity. Redirects,
  transient failures, and every other status never trigger this fallback.
- A sender run also caps one mutable-window snapshot at **1,000 records / 2 MiB encoded record
  data**. Larger local windows fail visibly before the first HTTP request instead of growing memory
  without bound or sending an incomplete authoritative replacement.
- Before reading an append page (at most 5,001 queried rows) or mutable snapshot (at most 1,001
  queried rows), Android performs a length preflight over that exact ordered, limited selection in
  the same database transaction. The conservative estimate charges every text value at six times
  its UTF-8 byte length for worst-case JSON escaping plus fixed per-row/object overhead. Oversized
  snapshots fail without truncation or cursor movement before unrestricted text enters app memory.
- Network or receiver failure must not block strap offload, local writes, analytics, or UI. Delivery
  is retried by the independent background worker.
- Android coalesces triggers that arrive during a running worker, processes at most one remembered
  device scope per attempt, and rotates that durable device cursor only after the slice completes.
  Per-call DNS and HTTP deadlines keep the attempt below WorkManager's execution window; automatic
  retries stop after a finite attempt budget and resume on a later successful offload or app launch.

## Identity and storage scope

`sourceId` is a random UUID generated locally and persisted for that app installation. `deviceId` is
the identifier already used by NOOP's local `device` table. Neither is an account or a globally
resolved user identity. The device name and MAC address are not part of this protocol.

A receiver must scope every row and idempotency record by at least:

```text
(sourceId, deviceId, stream, primary key)
```

Two installations that happen to use the same strap identifier must therefore not overwrite one
another. Reinstalling NOOP may create a new `sourceId`; reconciliation between installations is
deliberately outside v1.

Local progress is scoped by `(sourceId, normalized endpoint, selected protocolVersion,
receiverStateId)`. A changed endpoint, negotiated version, or receiver state ID forces a fresh
baseline. Rotating only the bearer token preserves progress. This prevents a newly initialized
receiver at the same URL from silently missing data and lets a protocol upgrade replay older append
records when their representation gains fields.
