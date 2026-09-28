# Training context

The Axum API owns the training read model: calendar days, units, NOOP source resolution, freshness and coverage ([ADR 0002](adr/0002-training-context-api-and-mcp.md)). The Hermes MCP adapter (`mcp/`, see `mcp/AGENTS.md`) sends each tool call to one API endpoint and returns the API's JSON unchanged. It never reads a database and has no rules of its own.

Reads today: the **Recovery Day** and **Workouts on a Training Day**. Both share the NOOP
[source resolution](#source-resolution) and [freshness and coverage](#freshness-and-coverage) rules.

## Recovery Day

`GET /api/training/days/{day}/recovery` with `Authorization: Bearer <API_KEY>`. MCP tool: `recovery_on_day(day)`.

`{day}` must be a Europe/Copenhagen calendar date written as canonical `YYYY-MM-DD`. The endpoint is read-only and draws only on the NOOP mirror (`noop.db`).

| Status | Meaning |
| --- | --- |
| 200 | The Recovery Day below. Days with no data are still 200, with null values. |
| 400 | `{day}` is not a canonical calendar date. |
| 401 | Missing or wrong API key. |
| 422 | The night has more than 24 session rows across both namespaces. The night is rejected, never truncated. |
| 500 | Storage error. |

Errors use the API's `{"error": "...", "status": <code>}` body. The MCP tool returns that same body as a tool error (`isError: true`).
The adapter rejects literal `.` and `..` arguments with a 400 tool error before URL construction, because URL libraries normalize those path segments. It does not follow API redirects.

### Response

This example is the 2026-09-15 night. The user edited the second session in NOOP, so the whole sleep block comes from the computed namespace.

```json
{
  "day": "2026-09-15",
  "time_zone": "Europe/Copenhagen",
  "day_start": "2026-09-15T00:00:00+02:00",
  "day_end": "2026-09-16T00:00:00+02:00",
  "sleep": {
    "total": { "value": 102.25, "unit": "min", "source": "my-whoop-noop" },
    "deep": { "value": 36.5, "unit": "min", "source": "my-whoop-noop" },
    "rem": { "value": 41.5, "unit": "min", "source": "my-whoop-noop" },
    "light": { "value": 24.25, "unit": "min", "source": "my-whoop-noop" },
    "efficiency": { "value": 0.2237417943, "unit": "fraction", "source": "my-whoop-noop" },
    "disturbances": { "value": null, "unit": "count", "source": null },
    "sessions": [
      {
        "start": "2026-09-14T23:58:00+02:00",
        "end": "2026-09-15T01:34:46+02:00",
        "detected_start": "2026-09-14T23:58:00+02:00",
        "duration_min": 96.76666666666667,
        "stage_min": { "awake": 0.0, "light": 49.5, "deep": 30.266666666666666, "rem": 17.0 },
        "efficiency_fraction": 1.0,
        "resting_hr_bpm": 47.0,
        "hrv_rmssd_ms": 88.2272585751,
        "user_edited": false,
        "staging_sparse": false,
        "source": "my-whoop-noop"
      },
      {
        "start": "2026-09-15T01:23:00+02:00",
        "end": "2026-09-15T09:00:00+02:00",
        "detected_start": "2026-09-15T01:23:00+02:00",
        "duration_min": 457.0,
        "stage_min": { "awake": 354.75, "light": 24.25, "deep": 36.5, "rem": 41.5 },
        "efficiency_fraction": 1.0,
        "resting_hr_bpm": 45.0,
        "hrv_rmssd_ms": 103.6740725999,
        "user_edited": true,
        "staging_sparse": false,
        "source": "my-whoop-noop"
      }
    ]
  },
  "resting_hr": { "value": null, "unit": "beats/min", "source": null },
  "hrv_rmssd": { "value": null, "unit": "ms", "source": null },
  "derived_scores": {
    "recovery": { "value": null, "unit": "score_0_100", "source": null },
    "strain": { "value": 25.92, "unit": "score_0_100", "source": "my-whoop-noop" }
  },
  "noop": {
    "installation_id": "81906e30-187d-4546-8f8a-9949b82d62fa",
    "imported_device_id": "my-whoop",
    "computed_device_id": "my-whoop-noop",
    "last_push_at": "2026-09-26T03:51:38+02:00",
    "freshness": "confirmed",
    "coverage": { "daily_metrics": "covered", "sleep_sessions": "covered" }
  }
}
```

- **Values**: every value is `{value, unit, source}`.
  - `source` is the NOOP device namespace the value was selected from. It is `null` exactly when `value` is `null`.
  - A null stays null: nothing is zero-filled or interpolated.
- **Timestamps**: ISO 8601, in Copenhagen local time, with the offset. `day_end` is exclusive.
- **Day bounds**: daylight saving time is honored. 2026-03-29 is 23 h long and 2026-10-25 is 25 h long.
- **`sleep`**: the Sleep Night that wakes on `day`.
  - The six summary values come from NOOP's daily row for `day`.
  - `sessions` holds the sessions whose `end` falls inside `[day_start, day_end)`, sorted by `start`.
  - `start` is the user's adjusted start if the session was edited, else `detected_start`.
  - `duration_min` is measured from `start`.
  - `stage_min` is null when the session has no staging.
- **`hrv_rmssd`**: NOOP's nightly RMSSD.
- **`derived_scores`**: NOOP's own model outputs, not measurements.
- **Excluded data**: raw streams, motion data, stage segments and sync bookkeeping are never returned.

## Workouts on a Training Day

`GET /api/training/days/{day}/workouts` with `Authorization: Bearer <API_KEY>`. MCP tool: `workouts_on_day(day)`.

`{day}` is a Europe/Copenhagen calendar date written as canonical `YYYY-MM-DD`. The endpoint is read-only. It reads the training log (`app.db`) and the NOOP mirror (`noop.db`).

| Status | Meaning |
| --- | --- |
| 200 | The day below. Days with nothing logged or recorded are still 200, with empty lists. |
| 400 | `{day}` is not a canonical calendar date. |
| 401 | Missing or wrong API key. |
| 422 | The day has more than 500 logged Set rows, or more than 24 NOOP workout rows across all namespaces. The day is rejected, never truncated. |
| 500 | Storage error. |

### Response

This example is 2026-09-26: the user logged a Workout that day and timed a session in NOOP overnight.

```json
{
  "day": "2026-09-26",
  "time_zone": "Europe/Copenhagen",
  "day_start": "2026-09-26T00:00:00+02:00",
  "day_end": "2026-09-27T00:00:00+02:00",
  "workouts": [
    {
      "id": 6,
      "name": "Pull",
      "notes": null,
      "exercises": [
        {
          "exercise_id": 11,
          "exercise": "Pull-up",
          "category": "calisthenics",
          "notes": null,
          "sets": [
            { "set_number": 1, "reps": 5, "added_weight_kg": 12.5, "hold_duration_s": null, "rpe": 10, "notes": "+2 partials" },
            { "set_number": 2, "reps": 5, "added_weight_kg": 12.5, "hold_duration_s": null, "rpe": 10, "notes": "+5 partials" }
          ]
        },
        {
          "exercise_id": 1,
          "exercise": "Planche Hold",
          "category": "calisthenics",
          "notes": "tuck",
          "sets": [
            { "set_number": 1, "reps": null, "added_weight_kg": null, "hold_duration_s": 15, "rpe": null, "notes": null }
          ]
        }
      ]
    }
  ],
  "noop_workouts": [
    {
      "start": "2026-09-26T02:12:38+02:00",
      "end": "2026-09-26T03:30:11+02:00",
      "sport": "Calisthenics",
      "origin": "manual",
      "duration_min": 77.55411666666667,
      "avg_hr_bpm": 81.0,
      "max_hr_bpm": 134.0,
      "strain_score": 24.51,
      "energy_kcal": 400.8306717632,
      "distance_m": null,
      "steps": null,
      "notes": null,
      "source": "my-whoop"
    }
  ],
  "noop": {
    "installation_id": "81906e30-187d-4546-8f8a-9949b82d62fa",
    "imported_device_id": "my-whoop",
    "computed_device_id": "my-whoop-noop",
    "last_push_at": "2026-09-26T03:51:38+02:00",
    "freshness": "partial",
    "coverage": { "workouts": "covered" }
  }
}
```

- **Two lists, never paired.** `workouts` are the Workouts logged in OpenHome with this date. `noop_workouts` are NOOP Workouts whose start falls in `[day_start, day_end)`. OpenHome Workouts are date-only, so nothing asserts that a NOOP Workout is a particular logged Workout. A NOOP session that starts after midnight belongs to the next Training Day, even if the logged Workout carries the previous date.
- **`workouts`**: in creation order. Exercises keep their logged order and Sets are ordered by `set_number`.
  - `added_weight_kg` is weight added to the body. It is null for a bodyweight Set.
  - `hold_duration_s` is a Timed Hold's duration. `rpe` is 1–10.
  - Missing fields are null. A Workout without Exercises has `exercises: []`.
- **`noop_workouts`**: ordered by `start`, with the values NOOP stored.
  - `duration_min` is NOOP's recorded duration, which may differ slightly from `end - start`.
  - `avg_hr_bpm` and `max_hr_bpm` are the stored values. NOOP's own screen may recompute them from the strap's heart-rate trace.
  - `strain_score` is NOOP's derived 0–100 workout strain: a model output, not a measurement.
  - `origin` is NOOP's classification of the row's `source`: `manual` (logged in NOOP), `detected` (a bout NOOP's detector derived from heart rate), `whoop` (WHOOP export), `health_import` (Apple Health or Health Connect, and NOOP's fallback for unknown sources), `lifting` (Hevy or Liftosaur) or `activity_file` (GPX, TCX or FIT).
  - `source` is the device namespace of the stored row.
  - HR zones, routes and raw streams are never returned.
- **An empty `noop_workouts` never means the user did not train.** With `coverage.workouts: covered`, NOOP recorded no workout starting that day. With `unknown`, rows may not have been pushed yet.

## Freshness and coverage

Every read's `noop` block tells how far the receiver's copy of the day can be trusted. A missing sync is never reported as a recorded null.

- **`last_push_at`**: the oldest of the latest completed replacements of the read's streams in every relevant namespace. For a Recovery Day that is `dailyMetric` and `sleepSession` in the two strap namespaces (four stream/namespace pairs). For Workouts on a Training Day it is `workout` in both strap namespaces and every other workout namespace known for the active installation, from retained rows or replacement windows. A replacement's completion time is the latest acceptance time of its parts. It is `null` until every pair has completed a replacement under the current receiver state. Staged parts, raw streams and other streams never advance it.
- **`freshness`** uses completed windows covering this particular day's rows (the Recovery Day's day and Sleep Night, or the Training Day's workout starts). A recent historical backfill cannot confirm a later day:
  - `confirmed`: every coverage rule of the read holds using only windows completed at or after `day_end`.
  - `partial`: the rules hold using windows completed at or after `day_start`, but not `day_end`.
  - `unconfirmed`: the rules hold only with older windows, or the read's watermark (`last_push_at`) predates `day_start`.
  - `unknown`: the receiver cannot establish any of the above.
- **`coverage.daily_metrics`** is `covered` when, in both namespaces, an applied `dailyMetric` replacement window contains `day`.
- **`coverage.sleep_sessions`** is `covered` when, in both namespaces, applied `sleepSession` windows jointly span session starts from the previous local midnight to `day_end`. This assumes no session lasts longer than a day.
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
  AND stream IN ('dailyMetric', 'sleepSession', 'workout')
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
