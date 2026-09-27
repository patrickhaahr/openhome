# Training context

The Axum API owns the training read model: calendar days, units, NOOP source resolution, freshness and coverage ([ADR 0002](adr/0002-training-context-api-and-mcp.md)). The Hermes MCP adapter (`mcp/`, see `mcp/AGENTS.md`) sends each tool call to one API endpoint and returns the API's JSON unchanged. It never reads a database and has no rules of its own.

The only read today is the **Recovery Day**.

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

### Freshness and coverage

`noop` tells how far the receiver's copy of the day can be trusted. A missing sync is never reported as a recorded null.

- **`last_push_at`**: the oldest of the latest completed `dailyMetric` and `sleepSession` replacements in both namespaces (four stream/namespace pairs). A replacement's completion time is the latest acceptance time of its parts. It is `null` until all four pairs have completed a replacement under the current receiver state. Staged parts, raw streams and other streams never advance it.
- **`freshness`** uses completed windows covering this particular day and Sleep Night. A recent historical backfill cannot confirm a later day:
  - `confirmed`: both coverage rules below hold using only windows completed at or after `day_end`.
  - `partial`: both rules hold using windows completed at or after `day_start`, but not `day_end`.
  - `unconfirmed`: both rules hold only with older windows, or the recovery watermark (`last_push_at`) predates `day_start`.
  - `unknown`: the receiver cannot establish any of the above.
- **`coverage.daily_metrics`** is `covered` when, in both namespaces, an applied `dailyMetric` replacement window contains `day`.
- **`coverage.sleep_sessions`** is `covered` when, in both namespaces, applied `sleepSession` windows jointly span session starts from the previous local midnight to `day_end`. This assumes no session lasts longer than a day.
- **What `covered` means**:
  - With `covered`, a missing row is a recorded absence.
  - With `unknown`, rows may simply not have been synced yet.
  - Staging or superseded windows never count.

### Source resolution

The API ports NOOP's own read-side precedence rules.

1. **Installation.** The active installation is the `source_id` of the latest strap batch (either namespace) accepted under the current `receiver_state`. Rows from other installations are ignored.
   - After a receiver-state rotation, and before the next push, `installation_id` is null. All values are then null, and freshness and coverage are `unknown`.
2. **Namespaces.** Only the imported strap namespace `my-whoop` and its computed sibling `my-whoop-noop` are read. These are fixed names. A strap that NOOP re-adds under another device id is not resolved.
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

### Inspecting the underlying rows

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
WHERE source_id = :source AND device_id IN ('my-whoop', 'my-whoop-noop')
  AND stream IN ('dailyMetric', 'sleepSession')
ORDER BY stream, device_id, start_inclusive;

-- Completion times behind freshness: the last accepted part of each applied replacement.
SELECT r.device_id, r.stream, r.start_inclusive, r.end_exclusive,
       MAX(l.accepted_at) AS completed_at
FROM replacement r
JOIN replacement_part p USING (receiver_state_id, source_id, device_id, replacement_id)
JOIN batch_ledger l USING (receiver_state_id, source_id, device_id, batch_id)
WHERE r.source_id = :source AND r.device_id IN ('my-whoop', 'my-whoop-noop')
  AND r.stream IN ('dailyMetric', 'sleepSession') AND r.state = 'applied'
GROUP BY r.device_id, r.stream, r.replacement_id;

-- Both namespaces' daily rows for the day.
SELECT device_id, total_sleep_min, efficiency, deep_min, rem_min, light_min, disturbances,
       resting_hr, avg_hrv, recovery, strain
FROM daily_metric
WHERE source_id = :source AND device_id IN ('my-whoop', 'my-whoop-noop') AND day = :day;

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

### Known limits

- **Push completeness**: the protocol has no completion signal for an entire phone sync. Freshness therefore establishes completion of the relevant replacement windows, not of the whole worker attempt. Unchanged windows that NOOP skips do not advance the watermark.
- **Ledger scan**: the installation lookup scans `batch_ledger`, which has no index on `accepted_at`. This is fine at the current volume.
