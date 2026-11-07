# Route search and Active Route lifecycle

Disposition for [#245](https://github.com/patrickhaahr/openhome/issues/245),
under API phase #241 and epic #240. Algorithms and public contracts remain as
specified in ADRs [0008](adr/0008-walking-loops-through-self-hosted-openrouteservice.md),
[0009](adr/0009-loop-alternatives-and-step-targets.md) and
[0010](adr/0010-persisted-active-route-selection.md).

## Search progress and termination

`route_loop::Provider::search` owns admission and termination. Cache hits bypass
the shared provider gate; misses wait at most 20 seconds for it and check the
cache again after admission. The elapsed search budget includes queue waiting.
No new attempt starts at or after 20 seconds; an admitted attempt retains its
own 10-second provider timeout, so the budget is not a hard response deadline.
Cancellation releases the gate without caching unfinished progress.

`SearchProgress` groups distinct retained loops, overshoot history and the
verdict used when no loop survives. The attempt loop produces its termination
reason directly, rather than updating independent termination flags:

| Exit | Result with retained loops | Result without retained loops |
| --- | --- | --- |
| Three matching distinct loops | `complete` | Impossible |
| All six seeds spent | `exhausted` | `unsuitable-route` if any candidate was unsuitable, otherwise `no-route` |
| Budget reached before an attempt, provider outage/refusal or invalid geometry | `interrupted` | `provider-unavailable`, overriding earlier unsuitable results |
| Origin-unroutable refusal before any loop is kept | Not applicable | Stop immediately with the existing empty verdict (including an earlier `unsuitable-route`) |

Other no-route refusals consume an attempt without clearing progress or the
empty verdict. Every valid geometry contributes its distance/requested-length
ratio to calibration before suitability or distinctness checks, including
duplicates and unsuitable loops. Matching loops still replace only retraced
near-misses, never matching loops. `SearchProgress::finish` owns empty-result
projection and final ordering/truncation: matches first, then absolute deviation,
at most three loops. It preserves partial results on interruption.

Only nonempty complete/exhausted results enter the existing 30-minute memory
cache. Interrupted results and failures retry on a later request. A queued
same-target read reuses a completed cached search; it starts another search
after an interrupted one. Target conversion, calibration bounds, geometry
measurement, distinctness, tolerance rounding and candidate identity are unchanged.

## Candidate retention and selection: retain SQL ownership

`active_route::remember`, `select` and `read` already own cohesive transactions
and row-presence rules. Retain them without another enum or persisted state:

- Generation remembers every offered plan before the authenticated handler
  returns it, including partial interrupted plans. Search-cache admission and
  durable candidate retention are separate decisions.
- Candidate row presence means selection eligibility. There is no age-based
  expiry in SQLite: expiration from the provider's memory cache does not make
  a stored candidate unselectable.
- Offering an existing identity moves its row to the newest offer position
  without rewriting its snapshot. The remember transaction retains the newest
  96 rows plus the selected row if it is older. It prunes other rows atomically.
- Unknown/pruned identities return `CandidateUnavailable` (HTTP conflict),
  leaving the selected route intact. A later generation may reoffer the same
  deterministic identity and make it eligible again.
- The singleton selection's first write checks candidate membership and sets
  identity/version together. Initial version is one; repeating the current
  identity preserves it; a different identity increments it. SQLite serializes
  competing replacements and retention writes. Snapshot decoding and commit
  remain in the selection transaction; failure rolls back replacement.
- The selected row remains protected indefinitely and readable across restart
  or provider outage. Replacing it removes that protection, but does not delete
  the former candidate immediately; a later remember may prune it. A phone's
  Route Session keeps its independent snapshot.

No migrations, checked SQL text or SQLx metadata changed.

## Validation handoff

No tests were edited or run, and no build/format/lint/check commands were run
for this implementation. GLM owns validation and commits. Existing proof is in
`api/tests/route_loop.rs`; coverage must be assessed rather than assumed.

Meaningful seam coverage includes concurrent same-target cache reuse; six-seed
exhaustion with zero/partial matches; provider failure and invalid geometry
before/after retained loops; queue time consuming the attempt budget; no attempt
after its boundary; calibration from unsuitable/duplicate geometry; matching
replacement and ordering; origin-unroutable refusal after an unsuitable result.

Against temporary migrated storage, verify newest-96 retention and reoffering,
selected-row protection, old-selection pruning after replacement, memory-cache
expiry independent of durable eligibility, unknown/pruned selection refusal,
idempotent selection, concurrent different-identity version increments, rollback
and restart reads. Retain authentication and sanitized-error assertions.
Use existing targeted just recipes and `just api-check` through `devenv shell --`;
integration and repository-wide gates remain with the orchestrator/GLM.
