# API cache lifecycle dispositions

Scope: [#244](https://github.com/patrickhaahr/openhome/issues/244), within
API phase #241 and epic #240. These caches remain independent; they have
different failure and concurrency contracts.

## Weather: explicit refresh admission

`api/src/services/weather.rs` owns the cache. `RefreshSchedule` distinguishes
an unattempted cache, a refresh scheduled after success, and failure backoff.
`is_due` admits the initial attempt immediately and later attempts at or after
their deadline. `Cache::finish_refresh` is the only refresh completion transition:

| Outcome | Forecast | Next admission |
| --- | --- | --- |
| Success | Replace with normalized response | Its `valid_until` (15 minutes after the sampled read time) |
| Failure, including timeout or invalid response | Retain the previous forecast, including absence | Sampled read time plus one minute |

The shared Tokio mutex guard owns the in-flight refresh, including the provider
await. Other reads wait and recheck admission under that guard; they do not
serve stale data concurrently with a refresh. There is no stored in-flight flag
to become stranded on cancellation: dropping the read releases the guard and
leaves the prior schedule and forecast intact. The provider timeout remains
five seconds, with redirects disabled.

`Cache::read` alone projects data to the existing public `WeatherRead`:
fresh while `now < valid_until`, otherwise stale while
`now < fetched_at + six hours`, otherwise unavailable. Empty caches are
unavailable after a failed attempt and during backoff. Failure never advances
the forecast timestamps. Freshness is derived on each read, independently of
refresh admission; it can cross the stale cutoff during backoff. Recovery
replaces both the forecast and the refresh schedule.

The read still samples UTC before waiting for the mutex and uses that same
instant for admission, provider normalization, retry deadlines and projection.
This preserves the existing behavior for queued reads, long provider calls and
wall-clock changes. Hourly selection still advances on each projection without
rewriting the cached forecast. No second internal freshness enum duplicates
the public projection. See also [ADR 0006](adr/0006-weather-provider-and-cache.md).

## Docker: retain timestamp invalidation

`DockerCache` in `api/src/lib.rs` and `api/src/routes/docker.rs` already express
the policy directly. An absent `last_updated` is invalid; a timestamp is stale
only when its age is **greater than** five seconds (equality remains a hit).
Successful start, stop and restart clear the timestamp; failed actions leave it
alone. Successful list fetches replace the containers and timestamp.

Listing releases the cache lock before provider IO, so concurrent misses can
fetch independently. Failures/timeouts return errors without serving cached
containers or installing backoff. An already-running list can repopulate after
an action invalidates the timestamp. Retain this existing ordering rather than
adding generation fences or Weather-style coalescing in a lifecycle refactor.

## Photo metadata: retain timestamp and membership checks

`api/src/services/photos.rs::album` checks the selected-album allowlist first.
A cached album is usable only while its monotonic age is strictly below
30 seconds. The shared metadata mutex stays held across the refresh and its
validation, coalescing successful concurrent reads, including media membership
checks. Expired metadata is removed **before** fetching. Failure or cancellation
leaves that entry absent; queued reads may retry immediately. There is no stale
membership fallback or retry backoff. Successful validation inserts the summary,
asset membership and capture times together with a new monotonic timestamp.
Retain this direct sequence: a separate lifecycle enum would add no invariant.

## Route cache: retain bounded timestamped entries

`api/src/services/route_loop.rs::Cache` keys results by activity and target metres.
Lookup removes all entries whose monotonic age is at least 30 minutes, then
clones a hit. At capacity (32 entries), insertion evicts the oldest stored entry;
hits do not renew timestamps. Provider searches share a gate and check the cache
again after acquiring it, so queued identical requests reuse a stored result.

Only nonempty, non-interrupted results are cached. Errors and interrupted
searches remain retryable on the next request; no stale fallback or negative
cache is introduced. A poisoned cache mutex behaves as a miss/skipped write.
Retain this small map implementation. Search termination and durable Active
Route retention belong to #245, outside this slice.
