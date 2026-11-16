# Training context: sleep and recovery

## Recent Sleep Nights

`GET /api/training/sleep/recent/{count}` with `Authorization: Bearer <API_KEY>`. MCP tool: `sleep_recent(n)`.

`count` and `n` are integers from 1 through 14. The API starts at today's Europe/Copenhagen wake day and returns exactly that many consecutive wake days, newest first. A day with no sleep data still has an entry. Counts outside the range return 400; missing or wrong credentials return 401. If any night exceeds the 24-row sleep-session cap, the whole request returns 422.

```json
{
  "time_zone": "Europe/Copenhagen",
  "nights": [
    {
      "wake_day": "2026-09-29",
      "wake_day_start": "2026-09-29T00:00:00+02:00",
      "wake_day_end": "2026-09-30T00:00:00+02:00",
      "sleep": {
        "total": { "value": null, "unit": "min", "source": null },
        "deep": { "value": null, "unit": "min", "source": null },
        "rem": { "value": null, "unit": "min", "source": null },
        "light": { "value": null, "unit": "min", "source": null },
        "efficiency": { "value": null, "unit": "fraction", "source": null },
        "disturbances": { "value": null, "unit": "count", "source": null },
        "sessions": []
      },
      "noop": {
        "installation_id": null,
        "imported_device_id": "my-whoop",
        "computed_device_id": "my-whoop-noop",
        "last_push_at": null,
        "freshness": "unknown",
        "coverage": { "daily_metrics": "unknown", "sleep_sessions": "unknown" }
      }
    }
  ]
}
```

Each entry uses the [Recovery Day](training-context-sleep-and-recovery.md#recovery-day) `sleep` fields and sleep/daily-metric `noop` status, including the same edited-sleep precedence, source labels, units, session timestamps, coverage, and freshness. Journal Entries and `coverage.journal` are exclusive to Recovery Day; journal rows do not affect this endpoint or its bounds. `wake_day_start` and `wake_day_end` are local midnight bounds with explicit offsets; they can span 23 or 25 hours at a daylight-saving change. The example shows one entry for brevity; a request for `n` returns exactly `n` entries.

## Recovery Day

`GET /api/training/days/{day}/recovery` with `Authorization: Bearer <API_KEY>`. MCP tool: `recovery_on_day(day)`.

`{day}` must be a Europe/Copenhagen calendar date written as canonical `YYYY-MM-DD`. The endpoint is read-only and draws only on the NOOP mirror (`noop.db`).

| Status | Meaning |
| --- | --- |
| 200 | The Recovery Day below. Days with no data are still 200, with null values. |
| 400 | `{day}` is not a canonical calendar date. |
| 401 | Missing or wrong API key. |
| 422 | The night has more than 24 session rows across both namespaces, or the day has more than 64 journal rows across all namespaces. The day is rejected, never truncated. |
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
        "stages": [
          { "start": "2026-09-14T23:58:00+02:00", "end": "2026-09-15T00:13:00+02:00", "stage": "deep" },
          { "start": "2026-09-15T00:13:00+02:00", "end": "2026-09-15T00:20:30+02:00", "stage": "light" },
          "..."
        ],
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
        "stages": [
          { "start": "2026-09-15T01:23:00+02:00", "end": "2026-09-15T01:59:30+02:00", "stage": "deep" },
          "..."
        ],
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
  "skin_temp_deviation": { "value": null, "unit": "degC", "source": null },
  "respiratory_rate": { "value": null, "unit": "breaths/min", "source": null },
  "spo2": { "value": null, "unit": "percent", "source": null },
  "steps": { "value": null, "unit": "count", "source": null },
  "derived_scores": {
    "recovery": { "value": null, "unit": "score_0_100", "source": null },
    "strain": { "value": 25.92, "unit": "score_0_100", "source": "my-whoop-noop" },
    "rest": { "value": 40.13, "unit": "score_0_100", "source": "openhome-derived" }
  },
  "journal": [
    { "question": "Alcohol", "answered_yes": true, "numeric_value": 2.0, "notes": null, "source": "noop-journal" },
    { "question": "Felt recovered", "answered_yes": false, "numeric_value": null, "notes": "Late night", "source": "noop-journal" }
  ],
  "noop": {
    "installation_id": "81906e30-187d-4546-8f8a-9949b82d62fa",
    "imported_device_id": "my-whoop",
    "computed_device_id": "my-whoop-noop",
    "last_push_at": "2026-09-26T03:51:38+02:00",
    "freshness": "confirmed",
    "coverage": { "daily_metrics": "covered", "sleep_sessions": "covered", "journal": "covered" }
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
  - `stages` is the session's stage segments in time order: `[{start, end, stage}]` with the same timestamp format as `start`, and `stage` one of `awake`, `light`, `deep` or `rem` (NOOP's `wake` is reported as `awake`). Segments are clamped to the session's `start` to `end`, so none begins before an edited start or runs past the end; empty ones are dropped. It is null when the session has no time-ordered staging (none stored, or only per-stage minutes), and then `stage_min` is all there is.
- **`hrv_rmssd`**: NOOP's nightly RMSSD.
- **`skin_temp_deviation`**: the night's skin temperature as a deviation from the user's baseline, in °C. It is not an absolute temperature.
- **`respiratory_rate`**: the night's respiratory rate, in breaths per minute.
- **`spo2`**: the day's blood oxygen saturation, in percent.
- **`steps`**: the day's step count.
- **`derived_scores`**: model outputs, not measurements.
  - `recovery` and `strain` are NOOP's own scores.
  - `rest` is NOOP's Rest composite (`sleep_performance`, 0-100) computed by OpenHome from the night's totals and the preceding sessions, so its source is `openhome-derived`. It weighs sleep against a personal need (50 %), efficiency (20 %), restorative sleep, the share of REM and deep (20 %), and the regularity of the last 28 nights (10 %). It is null when the day has no sleep or no efficiency. Where OpenHome lacks an input it follows NOOP's fallbacks: a need of 8 h under 7 nights of history, a neutral regularity of 0.5 under 3 nights, and an efficiency of 0.9 for a history night without one.
- **`journal`**: the Journal Entries the user logged in NOOP against `day` (felt recovered, alcohol, illness, stress, supplements, ...), sorted by `question`.
  - Each entry is `{question, answered_yes, numeric_value, notes, source}`. `source` is the namespace the entry was read from.
  - An entry keeps the day it was logged against and is never shifted. Entries logged against `day` often explain the Sleep Night that wakes on the next day, so read that day's Recovery Day beside them.
  - Entries are read from every namespace of the active installation with journal rows or journal replacement windows (today `my-whoop`, `my-whoop-noop`, `noop-app` and `noop-journal`). One entry is kept per question: `noop-journal` first, then `my-whoop`, then `my-whoop-noop`, then other namespaces alphabetically.
  - Check `noop.coverage.journal` before reading an empty list as "nothing logged". Journal pushes never change `last_push_at` or `freshness`.
- **Excluded data**: raw streams, motion data and sync bookkeeping are never returned.
