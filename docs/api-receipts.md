# API receipt and revision dispositions

Scope: [#246](https://github.com/patrickhaahr/openhome/issues/246), within API phase
#241 and epic #240. Each domain keeps its own receipt table and transaction; there is
no generic write engine. Their differences are deliberate and recorded here.

## Shared outcomes

A `request_id` is looked up before any destination or reference is resolved, so an
accepted retry cannot be failed by a since-changed default, list or Household User.
Then exactly one outcome applies, in this order:

| Outcome | Result |
| --- | --- |
| Unseen | The create proceeds and claims the receipt |
| Changed arguments | `409` conflict; the receipt is untouched |
| Target deleted | `409` conflict; the receipt stays, so an old retry never recreates it |
| Legacy receipt without a stored result (nutrition only) | `409` conflict directing the client to read first |
| Accepted | The domain's replay (`200`/`replayed: true`) |

Receipt claim, domain write and result capture commit in one transaction. A rejected
create rolls back with its claim, so it consumes no `request_id`. The claim
(`ON CONFLICT DO NOTHING`) serializes simultaneous retries: the loser rolls back and
replays the winner. Only if the receipt then cannot be read does it return a domain
conflict, and its wording differs: Exercise, Workout, Body Weight, Meal, Meal Log,
Supplement, Product link, Watchlist Entry, Binder Entry and Important Date say "Retry receipt
unavailable; ...", while Wishlist says
"The retried item was removed; use a new request_id to add it again".

Accepted-argument equality is domain-specific: each domain defines it by its own
receipt encoding or stored-column comparison, and omitted inputs can become explicit
nulls, defaults or empty arrays. Wishlist `details` serializes every key, with nulls;
receipts from before migration 0008 stay equivalent through that migration's backfill of
the default details.

## Domain table

| Domain | `request_id` | Replay returns | Deleted-target fence | Legacy without result |
| --- | --- | --- | --- | --- |
| Exercise | Optional | Current Exercise, `200` | Receipt `exercise_id` cleared (`SET NULL`; ids are reused) | n/a: no stored result |
| Workout | Optional | Current Workout with exercises, `200` | Receipt `workout_id` cleared | n/a |
| Body Weight | Optional | Current record, `200`; a new ID for a recorded date stays the duplicate-date conflict | Receipt `body_weight_id` cleared | n/a |
| Wishlist Item | Required | Current item, `replayed: true` | Item row missing (UUID id, no foreign key) | Migration 0008 backfills title-only details |
| Watchlist Entry | Required | Current entry within its owner | Entry row missing | n/a |
| Important Date | Required | Current date with figures | Date row missing | n/a |
| Binder Entry | Required | Original accepted entry, `replayed: true` | Entry row missing (UUID id, no foreign key) | n/a |
| Product link | Required | Original accepted Product | Product row missing ("no longer stored") | Conflict: look the Product up first |
| Meal Log | Required | Original accepted Meal Log | Meal Log row missing | Conflict: read the current meal log first |
| Meal | Required | Original accepted Meal | Meal row missing | Conflict: read the current meal first |
| Supplement | Required | Original accepted Supplement | Supplement row missing | Conflict: read the current supplement first |

"Original" means the result captured in the accepting transaction (`result`, migration
0025): later edits do not change a replay. "Current" replays read the live record, so
they reflect later edits. Fitness uses a nullable target id because its integer ids can
be reused; UUID-keyed domains need no cleared id because a missing row is unambiguous.
Fitness replay decisions live in `routes/fitness.rs::accepted_target`; each other
domain keeps its decision beside its own replay query and error type.

## Fitness client revisions

Workout edits and Profile edits take an optional `client_revision` (1 or greater,
otherwise a validation error, via `parse_client_revision`). The guarded statement
applies an edit only when the stored revision is null or older:

| Delivery | Result |
| --- | --- |
| Newer than stored, or first revision | Applies; stores the revision |
| Repeated or older | Changes nothing; answers `200` with the current record (acknowledged, not restored) |
| No revision | Always applies; the stored revision is left alone |
| Missing Workout | `404` from the current read (also the "no rows" path of a stale edit) |

Applying the edit and storing its revision are one statement (Profile) or one
transaction (Workout, including replacement of its exercises), so an edit never
applies without its revision.
