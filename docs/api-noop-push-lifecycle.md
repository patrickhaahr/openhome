# NOOP batch replay and receiver generation lifecycle

Disposition for [#249](https://github.com/patrickhaahr/openhome/issues/249),
under API phase #241 and epic #240. The wire contract stays
[`noop/PUSH_PROTOCOL.md`](noop/PUSH_PROTOCOL.md); storage meanings stay in
[`noop/DATA_MODEL.md`](noop/DATA_MODEL.md) (NOOP's on-device Swift/GRDB model, which does
not describe the receiver). Acknowledgements, status codes, error codes, stream
semantics and `noop.db` migrations are unchanged. `noop.db`, its pool and
`NOOP_PUSH_TOKEN` stay separate from `app.db` and `API_KEY`.

## Owners

| Decision | Owner |
| --- | --- |
| Receiver generation (`receiverStateId`) | `receiver_state` row; read inside each batch transaction in `ingest::apply`, and per request for capabilities |
| Unseen / identical retry / conflicting bytes | `ingest::BatchHistory::read`, from `batch_ledger` |
| Replace-window staging, apply, supersession | `replace_window::accept` over `replacement`, `replacement_scope`, `replacement_part` |
| Rotation cleanup | `receiver_state_rotation` trigger (migration `20260926001649`) |

## Batch decision

`apply` opens `BEGIN IMMEDIATE`, so concurrent deliveries serialize on the
write lock and each sees the other's committed ledger row. Within that
transaction it reads the generation, then classifies the batch against the
ledger scoped to `(receiver_state_id, source_id, device_id, batch_id)`:

| Ledger entry | Decision | Result |
| --- | --- | --- |
| None | `BatchHistory::Unseen` | Apply, then insert the ledger row |
| Same decoded-entity SHA-256 | `BatchHistory::Identical { ack }` | Replay the stored ack; no row changes |
| Different SHA-256 | Not a value: `read` returns `batch_conflict` (409) | Rollback; no row changes |

Content coding is never part of identity; the hash covers decoded bytes.
Conflict is an error rather than a third variant because no caller does anything
with it except reject.

An append batch either replays or upserts its records. The rows and the ledger
acknowledgement commit in the same transaction, so a failure leaves neither.
Stored acks are returned byte-identical, even across a restart.

A replace-window part is classified first, then handed to `replace_window::accept`
with the history. `accept` rejects a part of a superseded replacement first, even
when the history is `Identical`; for any non-superseded replacement it answers
`PartOutcome::Retry` exactly when the history is `Identical`. `apply` treats a retry without a ledger entry as an internal
error rather than guessing. `Staged` and `Applied` both continue to the ledger
insert, so the ack for the completing part is only stored and returned after the
atomic apply succeeded.

## Receiver generation and rotation

The ledger, `replacement`, `replacement_scope` and `replacement_part` rows are
all keyed by `receiver_state_id`. Rotation is an operator `UPDATE` of
`receiver_state`; the trigger deletes all four tables in that same statement's
transaction and leaves the stream tables untouched. A batch transaction either
observes the old ID with the old metadata or the new ID with none, so no
acknowledgement, staged payload or fence survives into the new generation, and a
retained health row is overwritten or reconciled by the next baseline.

Backups must therefore copy the whole `noop.db` (receiver state, stream tables,
ledger, replacement metadata and staged entities) with the API stopped or with
SQLite's online backup mechanism. Restoring a stale backup is the case that needs
a rotated ID.

## Replacement lifecycle: retained

The persisted `replacement.state` (`staging`, `applied`, `superseded`) and the
single current replacement per `(source, device, stream)` already own the
lifecycle, with migration `CHECK` constraints on state. It is not replaced by a
Rust state enum or another column. In source:

| State | Entered by | Parts and retries |
| --- | --- | --- |
| `staging` | First part of a replacement id (`begin_generation`) | New parts stage; identical retries replay |
| `applied` | Part that completes the set | Identical retries replay; any other part is `replacement_conflict` (409) |
| `superseded` | First part of a different replacement id in the scope while the current one is `staging` | Every part, including a byte-identical retry of one that was accepted, is `replacement_superseded` (409) |

An applied replacement stays current in `replacement_scope`, and a retry of a
known applied part replays before it can alter the scope, so a late retry never
overwrites newer data. Staged `entity` bytes are cleared on apply and on
supersession; part rows keep their `batch_id` to detect part-number reuse.

## Protocol reconciliation and receiver behavior

PUSH_PROTOCOL.md ("Authoritative rolling-window delivery") says retrying any part
is harmless, and also that a receiver rejects late parts of a superseded generation
with `409`. The receiver reconciles them as follows: a retry is harmless in that
it never changes rows, the ledger or the current generation, and an accepted part
replays its ack while its replacement is `staging` or `applied`. Once the
replacement is `superseded`, even a byte-identical retry of an accepted part is a
late part of a superseded generation and gets `replacement_superseded` (409). This
follows the superseded-generation clause; it is not a deviation.

Receiver implementation behavior the protocol does not specify, not deviations:

- Ledger and replacement rows are never pruned; they only disappear on rotation.
- The batch transaction reads the receiver generation itself, while `GET` reads it
  separately. A rotation between a sender's capability read and its `POST` is
  handled with the new generation.

## Existing proof

`api/tests/noop_push.rs` covers byte-identical and cross-encoding retries,
conflicts, per-source/device scoping, concurrent identical posts, concurrent
differing-byte deliveries of one `batchId`, out-of-order parts, staged parts
across reconnects, supersession and late parts, retries of applied
replacements, and rotation of the append and replacement metadata. The
differing-bytes concurrency test was added for #249; #249 changed no behavior.
