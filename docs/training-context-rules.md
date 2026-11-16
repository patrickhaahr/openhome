# Training context: shared rules

## Freshness and coverage

The Recovery Day, Workouts on a Training Day and NOOP Workout Detail reads have a `noop` block that tells how far the receiver's copy of the day can be trusted. A missing sync is never reported as a recorded null.

- **`last_push_at`**: the oldest of the latest completed replacements of the read's streams in every relevant namespace. For a Recovery Day that is `dailyMetric` and `sleepSession` in the two strap namespaces (four stream/namespace pairs). For Workouts on a Training Day it is `workout` in both strap namespaces and every other workout namespace known for the active installation, from retained rows or replacement windows. A replacement's completion time is the latest acceptance time of its parts. It is `null` until every pair has completed a replacement under the current receiver state. Staged parts, raw streams and other streams never advance it.
- **`freshness`** uses completed windows covering this particular day's rows (the Recovery Day's day and Sleep Night, or the Training Day's workout starts). A recent historical backfill cannot confirm a later day:
  - `confirmed`: every coverage rule of the read holds using only windows completed at or after `day_end`.
  - `partial`: the rules hold using windows completed at or after `day_start`, but not `day_end`.
  - `unconfirmed`: the rules hold only with older windows, or the read's watermark (`last_push_at`) predates `day_start`.
  - `unknown`: the receiver cannot establish any of the above.
- **`coverage.daily_metrics`** is `covered` when, in both namespaces, an applied `dailyMetric` replacement window contains `day`.
- **`coverage.sleep_sessions`** is `covered` when, in both namespaces, applied `sleepSession` windows jointly span session starts from the previous local midnight to `day_end`. This assumes no session lasts longer than a day.
- **`coverage.journal`** (Recovery Day only) is `covered` when every journal namespace of the active installation (one with journal rows or journal replacement windows) has an applied `journal` replacement window containing `day`. With no journal namespace it is `unknown`. Recent Sleep Nights and the Training Context have no `coverage.journal`.
- **`coverage.workouts`** is `covered` when, in every relevant workout namespace, applied `workout` windows jointly span workout starts from `day_start` to `day_end`. A discovered import with only partial or staged coverage keeps the result unknown, even when the strap windows are complete.
- **What `covered` means**:
  - With `covered`, a missing row is a recorded absence.
  - With `unknown`, rows may simply not have been synced yet.
  - Staging or superseded windows never count.

## Source resolution

The API ports NOOP's own read-side precedence rules.

1. **Installation.** The active installation is the `source_id` of the latest strap batch (either namespace) accepted under the current `receiver_state`. Rows from other installations are ignored.
   - After a receiver-state rotation, and before the next push, `installation_id` is null. All NOOP values are then null or empty, and freshness and coverage are `unknown`.
2. **Namespaces.** Recovery reads the imported strap namespace `my-whoop` and its computed sibling `my-whoop-noop`. These are fixed names; recovery does not resolve a strap that NOOP re-adds under another device id. Workouts read every namespace of the active installation, including `apple-health`, `health-connect`, `lifting` and `activity-file`, and retain the actual device id as `source`.
3. **Daily values.** Each field is taken from the imported value first, with the computed value filling nulls.
   - When both daily rows exist, the whole sleep block (`total`, `deep`, `rem`, `light`, `efficiency`, `disturbances`) comes from the computed row instead, nulls included. This happens if either:
     - a computed session of the night is user-edited (edited sleep wins), or
     - the imported row is a bare aggregate (a total without efficiency or stages) and the computed row has a total and is not bare.
4. **Sessions.** The night keeps all imported sessions unless the computed sessions are strictly richer. In that case it keeps all computed sessions instead.
   - A night's richness is the best richness of its sessions:
     - 0: no staging.
     - 1: staging covers under 95% of the detected span.
     - 2: fuller staging, or staging whose coverage cannot be measured.
   - Computed sessions are used when there are no imported ones.
5. **Workouts.** All namespaces' rows form one list, as in NOOP's workout list (`dedupCrossSource`):
   - A `detected` row is dropped when it overlaps a non-detected row by more than half of the shorter of the two, whatever the sports.
   - Two rows of the same sport that overlap by more than half of the shorter one are one activity. Sports match case- and space-insensitively, and `detected` matches `Activity`. The row with more captured signals wins (average HR, max HR, strain, zones, distance above 0, energy above 0). On a tie a non-`health_import` row wins, then the longer session, then the row read first (imported strap, computed strap, then other device ids alphabetically; within each namespace, start).
   - Every other row stays a distinct NOOP Workout, including back-to-back sessions of one sport.

## Inspecting the underlying rows

To investigate a discrepancy, open the mirror read-only: `sqlite3 -readonly api/data/noop.db`, or the `NOOP_DB_URL` file.

Take `:source` from `noop.installation_id`. Take `:day`, `:day_start` and `:day_end` from the response. SQLite's `unixepoch()` understands the offsets.

```sql
-- Latest accepted push per installation and namespace under the current receiver state.
SELECT l.source_id, l.device_id, MAX(l.accepted_at) AS latest
FROM batch_ledger l
JOIN receiver_state r ON r.id = 1 AND r.receiver_state_id = l.receiver_state_id
WHERE l.device_id IN ('my-whoop', 'my-whoop-noop')
GROUP BY l.source_id, l.device_id
ORDER BY latest DESC;

-- Replacement windows behind coverage (only 'applied' counts).
SELECT device_id, stream, start_inclusive, end_exclusive, parts, state
FROM replacement
WHERE source_id = :source
  AND stream IN ('dailyMetric', 'sleepSession', 'workout', 'journal')
ORDER BY stream, device_id, start_inclusive;

-- Completion times behind freshness: the last accepted part of each applied replacement.
SELECT r.device_id, r.stream, r.start_inclusive, r.end_exclusive,
       MAX(l.accepted_at) AS completed_at
FROM replacement r
JOIN replacement_part p USING (receiver_state_id, source_id, device_id, replacement_id)
JOIN batch_ledger l USING (receiver_state_id, source_id, device_id, batch_id)
WHERE r.source_id = :source
  AND r.stream IN ('dailyMetric', 'sleepSession', 'workout') AND r.state = 'applied'
GROUP BY r.device_id, r.stream, r.replacement_id;

-- All namespaces' Journal Entries for the day, before one is kept per question.
SELECT device_id, question, answered_yes, numeric_value, notes
FROM journal
WHERE source_id = :source AND day = :day
ORDER BY question, device_id;

-- Both namespaces' daily rows for the day.
SELECT device_id, total_sleep_min, efficiency, deep_min, rem_min, light_min, disturbances,
       resting_hr, avg_hrv, recovery, strain
FROM daily_metric
WHERE source_id = :source AND device_id IN ('my-whoop', 'my-whoop-noop') AND day = :day;

-- All namespaces' workouts of a Training Day, before selection.
SELECT device_id, sport, source, datetime(start_ts, 'unixepoch') AS start_utc,
       datetime(end_ts, 'unixepoch') AS end_utc, duration_s, avg_hr, max_hr, strain,
       energy_kcal, distance_m, steps, zones_json IS NOT NULL AS has_zones
FROM workout
WHERE source_id = :source
  AND start_ts >= unixepoch(:day_start) AND start_ts < unixepoch(:day_end)
ORDER BY device_id, start_ts;

-- The strap's heart rate samples of a NOOP Workout, its window and 315 s after it. Take :start
-- and :end from the NOOP Workout Detail.
SELECT datetime(ts, 'unixepoch') AS at_utc, bpm
FROM hr_sample
WHERE source_id = :source AND device_id = 'my-whoop'
  AND ts >= unixepoch(:start) AND ts <= unixepoch(:end) + 315
ORDER BY ts;

-- Both namespaces' sessions of the Sleep Night, before selection.
SELECT device_id, datetime(start_ts, 'unixepoch') AS start_utc,
       datetime(start_ts_adjusted, 'unixepoch') AS adjusted_start_utc,
       datetime(end_ts, 'unixepoch') AS end_utc, user_edited, staging_sparse, efficiency,
       resting_hr, avg_hrv, length(stages_json) AS stages_bytes
FROM sleep_session
WHERE source_id = :source AND device_id IN ('my-whoop', 'my-whoop-noop')
  AND end_ts >= unixepoch(:day_start) AND end_ts < unixepoch(:day_end)
ORDER BY device_id, start_ts;
```

## Known limits

- **Push completeness**: the protocol has no completion signal for an entire phone sync. Freshness therefore establishes completion of the relevant replacement windows, not of the whole worker attempt. Unchanged windows that NOOP skips do not advance the watermark.
- **Workout dismissals**: NOOP hides detected bouts the user dismissed, but dismissals are not pushed. The API can still return a dismissed `detected` bout. Workout sources that have never reached the mirror cannot be discovered or included in coverage.
- **Ledger scan**: the installation lookup scans `batch_ledger`, which has no index on `accepted_at`. This is fine at the current volume.
