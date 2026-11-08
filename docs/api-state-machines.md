# API lifecycle dispositions

Consolidated record for [#251](https://github.com/patrickhaahr/openhome/issues/251),
API phase [#241](https://github.com/patrickhaahr/openhome/issues/241) and epic
[#240](https://github.com/patrickhaahr/openhome/issues/240). It maps every API
inventory candidate to a disposition, and links the slice documents or source
modules that own the detail. It records no gate results: those belong to the issues
and to the commands that produced them.

The approach was explicit, per-lifecycle ownership with Rust enums only where they
remove ambiguity; no state-machine library, shared workflow framework or generic
write engine was added. Independent facts (policy, transport, command, observation)
stay independent rather than being enumerated as combinations.

Dispositions: **refactored** (code changed to make transitions explicit),
**already explicit** (the existing model was the clearest correct form, now
recorded), **retained simple** (a direct branch, transaction or sequence kept, with
a rationale), **deferred** (with a linked explanation).

## Inventory

| Candidate | Slice | Disposition | Rationale and owner |
| --- | --- | --- | --- |
| Backlight Automatic / Bedtime / manual-override policy | [#242](https://github.com/patrickhaahr/openhome/issues/242) | Refactored | `backlight::routine::Policy` consumes the current policy and yields the next `Target` (select, Automatic, Bedtime edit, time-driven resolve, startup restore); a manual selection carries its own data, so source and override cannot disagree. Policy is independent of delivery. See [ADR 0011](adr/0011-local-backlight-control-and-bedtime-policy.md), [backlight.md](backlight.md) |
| Backlight durable dispatch, supersession, retry/reconnect, profile readiness, ordered Bluetooth exchange | [#243](https://github.com/patrickhaahr/openhome/issues/243) | Refactored | `backlight::delivery` holds every transition as an I/O-free method on `Status`; `command`, `recovery`, `reachability`/`observed` and `acknowledged` stay separate facts. The service only sequences persist, exchange and record. One GATT exchange is ordered writes followed by correlated queries; an accepted command, a completed write and a device observation remain distinct. See [backlight.md](backlight.md) |
| Weather refresh/backoff/freshness | [#244](https://github.com/patrickhaahr/openhome/issues/244) | Refactored | `RefreshSchedule` and `Cache::finish_refresh` own admission and completion; the public projection stays derived. [api-cache-lifecycles.md](api-cache-lifecycles.md), [ADR 0006](adr/0006-weather-provider-and-cache.md) |
| Docker cache | #244 | Retained simple | A nullable timestamp compared against a 5 s age already states the policy; concurrent misses fetching independently is the existing contract. [api-cache-lifecycles.md](api-cache-lifecycles.md) |
| Photo metadata cache | #244 | Retained simple | Allowlist check, strict 30 s age and a mutex held across refresh coalesce reads; an enum adds no invariant. [api-cache-lifecycles.md](api-cache-lifecycles.md) |
| Route result cache | #244 | Retained simple | A bounded, timestamped map (32 entries, 30 min). [api-cache-lifecycles.md](api-cache-lifecycles.md) |
| Bounded route search termination | [#245](https://github.com/patrickhaahr/openhome/issues/245) | Refactored | The attempt loop yields its termination reason directly; `SearchProgress` owns retained loops, calibration history and empty-result projection. [api-route-lifecycles.md](api-route-lifecycles.md) |
| Route candidate retention / Active Route selection | #245 | Already explicit | Existing SQL transactions and candidate-row presence own eligibility, newest-96 retention and the protected selection; another enum or persisted state would duplicate them. [api-route-lifecycles.md](api-route-lifecycles.md), [ADR 0010](adr/0010-persisted-active-route-selection.md) |
| Fitness create-receipt replay and deletion fence | [#246](https://github.com/patrickhaahr/openhome/issues/246) | Refactored | Fitness replay decisions are extracted into `routes/fitness.rs::accepted_target`; its nullable target id fences deleted targets because integer ids are reused. [api-receipts.md](api-receipts.md) |
| Other durable create receipts (Wishlist, Watchlist, Important Date, Product link, Meal Log, Meal, Supplement) | #246 | Retained simple | Each domain keeps its own receipt table, transaction and replay query beside its error type; the five shared outcomes and per-domain differences are recorded, with no generic write engine. [api-receipts.md](api-receipts.md) |
| Fitness client-revision acceptance | #246 | Refactored | Newer/first applies, repeated/older acknowledges the current record, absent never touches the stored revision; apply and revision are one statement or transaction. [api-receipts.md](api-receipts.md) |
| Meal photo encoding, filesystem/database ownership and cleanup | [#247](https://github.com/patrickhaahr/openhome/issues/247) | Retained simple | Caller-side compensation stays; encoded, stored and referenced ownership and the crash/cancellation windows are described in the `meal_photos` module documentation, with no collector or distributed transaction. |
| Nutrition week eligibility, provisional computation, first-read freezing | [#248](https://github.com/patrickhaahr/openhome/issues/248) | Refactored | `freeze_weeks` (renamed) now owns insertion and rereading of only the requested records, so callers receive stored values, never an eligible computation relabelled as fixed. Row presence remains the frozen state; no status column or enum was added. [api-nutrition-week-lifecycle.md](api-nutrition-week-lifecycle.md), [ADR 0014](adr/0014-weekly-target-record-fixed-on-first-read.md) |
| NOOP batch acceptance, replay, receiver generation | [#249](https://github.com/patrickhaahr/openhome/issues/249) | Refactored | `BatchHistory` makes unseen / identical retry explicit (a conflicting body is the `batch_conflict` error). [api-noop-push-lifecycle.md](api-noop-push-lifecycle.md) |
| NOOP replacement staging/applied/superseded | #249 | Already explicit | The persisted `replacement.state` and per-scope current replacement own the lifecycle. [api-noop-push-lifecycle.md](api-noop-push-lifecycle.md) |
| RSS scheduler | [#250](https://github.com/patrickhaahr/openhome/issues/250) | Refactored | The startup-fetch-then-sleep loop moved verbatim from `main` to `feed::run_scheduler` (source: `api/src/services/feed.rs`), with named `REFRESH_INTERVAL` (24 h) and `ERROR_PAUSE` (60 s) constants. Semantics are unchanged: one startup pass, then a 24-hour sleep before each pass; a scheduled pass that returns `Err` (only when the Feed list fails to load) sleeps 60 s more and is followed by the normal wait, not an immediate retry; a failed initial pass has no extra pause; manual refreshes do not move the schedule |
| RSS conditional fetch and feed metadata | #250 | Retained simple | `feed::refresh_feed` stays one sequential conditional GET (stored `ETag`/`Last-Modified` sent as `If-None-Match`/`If-Modified-Since`). Only HTTP-status and parse failures record `last_error`; URL, DNS, request, body-read and size failures leave the row untouched and only return the error. 304 sets `last_fetched_at`, clears `last_error` and keeps validators; a parsed 2xx stores the response validators, clearing one the response lacks. Items deduplicate by unique violation. A persistent state model would add nothing to this sequence |

## Preserved contracts

HTTP schemas, status codes, authentication, retry identities and stored data are
unchanged by every slice above; a changed contract would have needed its own
scope. Applied migrations are unchanged. Where a slice touched checked queries,
its issue records the SQLx metadata handling; this document does not restate it.

| Area | Preserved externally observable behavior |
| --- | --- |
| Backlight | Selections are persisted before Bluetooth, and only the latest pending target is sent; a superseded multi-frame upload stops before its remaining frames, but an exchange that already had writes accepted stays acknowledged while only the current revision updates `command` and `recovery`. Off is selectable without a captured TV Sync profile and without Bluetooth, but device unavailability is reported and acceptance is not claimed (other selections need a profile). Private packets and identifiers never enter the HTTP contract; status keeps command, recovery and fresh/stale observation distinct |
| Weather, caches | Weather keeps the fresh / stale (six hours) / unavailable projection and a five-second provider timeout; a failed refresh retains the previous forecast (including none) and delays the next attempt by one minute. Separately, Docker lists expire after five seconds, route results after 30 minutes and photo metadata after 30 seconds; Docker and route errors return without a stale fallback or negative cache, and interrupted route searches are not cached |
| Routes | Public outcomes, 20 s search budget, three-loop limit, newest-96 candidate retention, protected selection across restart |
| Receipts, revisions | `request_id` replay, conflict and deleted-target fence semantics; `client_revision` acknowledgement of repeated or older edits |
| Meal photos | Re-encoding, ICC conversion, dimensions, metadata stripping, privacy and API errors |
| Nutrition weeks | Frozen record never updated; provisional weeks stored nowhere |
| NOOP push | Wire protocol, acknowledgements, status and error codes, separate `noop.db`, token and migrations |

## Behavior proven by existing tests

Only behavior with an accepted test in the repository is listed; this is a map to
the suites, not a record of a run.

- **Backlight** (`api/tests/backlight.rs`): rapid clients cancel partial uploads and
  keep only the latest target (`rapid_effect_clients_cancel_partial_uploads_and_keep_only_the_latest_target`);
  an outage retains the stale observation and recovers only the latest target under
  concurrent commands (`outage_retains_stale_observation_and_recovers_only_latest_target_during_concurrent_commands`);
  silent reconnect reapplies the current target while a healthy refresh does not;
  a command that expires while queued never reaches hardware; a superseded exchange
  that finished writing stays acknowledged; disconnecting after durable acceptance
  completes the accepted work; restart recovers the current window and only
  unexpired overrides; failed transition persistence is retried with the current
  clock before any device write.
- **Routes** (`api/tests/route_loop.rs`): concurrent same-target cache reuse,
  interrupted partial results, retention and selection protection, restart reads.
- **Receipts** (`fitness_*_retries.rs`, `fitness_profile_revisions.rs`,
  `nutrition_meal_logs.rs`, domain suites): concurrent claims, conflict precedence,
  deletion fences, legacy results, revision acknowledgement.
- **Meal photos** (`api/tests/nutrition_meal_logs.rs`): rejected photos write nothing,
  replay and conflict, attach/replace/remove, and refusal/cleanup against real file
  databases. Power-loss durability and dropped-future cleanup are **not** established;
  see the module limits.
- **Nutrition weeks** (`api/tests/nutrition_targets.rs`): racing first reads return one
  stored winner (`concurrent_first_reads_agree_on_the_stored_winner`); a stored
  week survives Profile loss; the provisional/frozen boundary.
- **NOOP push** (`api/tests/noop_push.rs`): byte-identical and cross-encoding retries,
  conflicts, concurrent identical and differing-byte deliveries, out-of-order parts,
  staged parts across reconnects, supersession and late parts, rotation, backup and
  restart convergence (`baseline_retry_converges_after_restart_and_database_backup`).
- **Weather** (`api/tests/weather.rs`) and **cache lifecycles**: see
  [api-cache-lifecycles.md](api-cache-lifecycles.md).

## Outstanding gates and limits

These are not demonstrated by automated API checks and stay pending unless a
separate acceptance records them:

- **Physical Bluetooth.** Automated Backlight tests use a transport seam. LED color
  and brightness on hardware, link behavior, and recovery against a real controller
  are verified only by the recorded acceptance in
  [backlight-local-verification.md](backlight-local-verification.md),
  [backlight-effects.md](backlight-effects.md) and
  [backlight-production-verification.md](backlight-production-verification.md);
  this phase performed no new device command.
- **Providers.** Open-Meteo, the routing provider, Open Food Facts, TMDB, Immich and
  other live upstreams were not called as part of this phase.
- **Production.** No deployment, household-database migration or reset, or
  household-device mutation belongs to this phase.
- **Calendar.** Nutrition week tests compare against the live Copenhagen calendar;
  a future week's own handover to frozen happens when that week begins and is not
  simulated ([api-nutrition-week-lifecycle.md](api-nutrition-week-lifecycle.md)).
- **RSS.** The production fetch seam requires an `https` URL resolving to a global
  address and follows no redirects. The existing `feed_refresh` suite covers invalid,
  non-HTTPS and private-host rejection and the storage-level per-feed item
  deduplication contract, but no automated test drives a successful network fetch,
  `ETag`/`Last-Modified` revalidation, 304 handling or metadata/error updates through
  `refresh_feed`, and none covers the scheduler's startup pass, 24-hour wait or
  60-second error pause. That is a coverage gap, not a statement that such proof is
  impossible or required. Real 24-hour waits and external feeds are out of scope.
- **Gate status.** The integrated API verification gate and the issue/checkbox state
  for #249, #250, #251 and the phase are recorded on those issues, not in this file.

## Deferred sections

[#240](https://github.com/patrickhaahr/openhome/issues/240) defers, with their own
future bounded acceptance criteria and no work started here:

- **Dashboard**: Backlight queue/debounce, save-and-refresh outcomes, browser-held
  retries, slideshow/deck playback, idle/wake, pagination, weather polling, map
  resources and gestures. Dispositions are in
  [dashboard-lifecycles.md](dashboard-lifecycles.md).
- **Native Android (`mobile/`)**: setup, immediate Home commands, Active Workout,
  local saves and correction, outbox/WorkManager delivery, rest alarms.
- **TV and firmware**: launcher navigation and key holds, command feedback, polling,
  picture-profile readback, bridge cache/admission, process supervision, sequential
  firmware actuation.

`mobile-expo/` was removed by the native Android merge and is excluded. The CLI and
MCP remain typed API consumers and keep no client-side copy of these lifecycles.
Because HTTP schemas are preserved, those refactors stay independent of this phase.
