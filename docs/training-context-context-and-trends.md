# Training context: Training Context and metric trends

## Training Context

`GET /api/training/context/{from_day}/{to_day}` with `Authorization: Bearer <API_KEY>`. MCP tool: `training_context(from_day, to_day)`.

A compact view of a training block for a programming discussion. Dates are inclusive, canonical `YYYY-MM-DD` Copenhagen calendar days, and the range must contain 1–90 days. It reads the training log (`app.db`) and the NOOP mirror (`noop.db`).

| Status | Meaning |
| --- | --- |
| 200 | One entry per calendar day, oldest first. A day with nothing logged or synced still has an entry. |
| 400 | Invalid date, reversed range, or more than 90 days. |
| 401 | Missing or wrong API key. |
| 422 | More than 1,000 logged Exercise entries in the range (a Workout without Exercises counts as one), more than 24 NOOP workout rows starting on one day, or a JSON response larger than 512 KiB (524,288 bytes). The block is rejected, never truncated. |

```json
{
  "from_day": "2026-09-28",
  "to_day": "2026-09-30",
  "time_zone": "Europe/Copenhagen",
  "noop": {
    "installation_id": "81906e30-187d-4546-8f8a-9949b82d62fa",
    "imported_device_id": "my-whoop",
    "computed_device_id": "my-whoop-noop",
    "last_push_at": { "recovery": "2026-10-02T04:00:00+02:00", "workouts": "2026-10-02T04:00:00+02:00" }
  },
  "days": [
    {
      "day": "2026-09-28",
      "workouts": [
        {
          "id": 6,
          "name": "Pull",
          "exercises": [
            { "exercise": "Pull-up", "sets": 2, "best_reps": 5, "best_added_weight_kg": 12.5,
              "best_hold_duration_s": null, "best_rpe": 10, "volume_kg": 112.5 },
            { "exercise": "Planche Hold", "sets": 2, "best_reps": null, "best_added_weight_kg": null,
              "best_hold_duration_s": 15, "best_rpe": 9, "volume_kg": 0.0 }
          ]
        }
      ],
      "noop_workouts": [],
      "body_weight": { "value": 80.0, "unit": "kg", "source": "body_weight" },
      "sleep_duration": { "value": 420.0, "unit": "min", "source": "my-whoop" },
      "resting_hr": { "value": 50.0, "unit": "beats/min", "source": "my-whoop" },
      "hrv_rmssd": { "value": 62.0, "unit": "ms", "source": "my-whoop-noop" },
      "noop": {
        "recovery": {
          "freshness": "confirmed",
          "coverage": { "daily_metrics": "covered", "sleep_sessions": "covered" }
        },
        "workouts": { "freshness": "confirmed", "coverage": { "workouts": "covered" } }
      }
    }
  ]
}
```

The example shows one day for brevity; the response has an entry for each day in the range.

- **Two lists, never paired.** `workouts` and `noop_workouts` follow the [Workouts on a Training Day](training-context-workouts.md#workouts-on-a-training-day) rules: logged Workouts carry their date, and NOOP Workouts belong to the local day their start falls on, after NOOP's cross-source deduplication. Nothing asserts that a NOOP Workout is a particular logged Workout.
- **`workouts`**: in creation order, with Exercise entries in logged order; repeated entries of one Exercise stay separate. Each entry reduces its Sets to counts and bests: `sets` (count), `best_reps`, `best_added_weight_kg` (null when every Set was bodyweight), `best_hold_duration_s` (longest Timed Hold), `best_rpe`, and `volume_kg`. `volume_kg` follows the Volume rule in `CONTEXT.md`: reps times Added Weight, bodyweight Sets add zero, and it is null for an entry without Sets. For a weighted Timed Hold it is hold seconds times kg, as in the Progress Series. Exact Sets come from `exercise_history` or `workouts_on_day`.
- **`noop_workouts`**: by start, with `start`, `end`, `sport`, `origin`, `duration_min`, `avg_hr_bpm`, `strain_score`, `steps` and `source`. Other stored metrics, HR zones and routes are left to `workouts_on_day` and the NOOP workout detail.
- **`body_weight`**: only the daily Body Weight log. Its `source` is `body_weight` when a record exists; it has no freshness because it is entered directly.
- **`sleep_duration`, `resting_hr`, `hrv_rmssd`**: the [Recovery Day](training-context-sleep-and-recovery.md#recovery-day) values for the Sleep Night waking on the day, with the same precedence, including edited sleep. Sleep sessions are left to `recovery_on_day`.
- **`noop`**: each day has one block per NOOP read. `noop.recovery` is the Recovery Day's `freshness` and `coverage` for that day, and applies to `sleep_duration`, `resting_hr` and `hrv_rmssd`. `noop.workouts` is Workouts on a Training Day's, and applies to `noop_workouts`. The top-level `last_push_at.recovery` and `last_push_at.workouts` are those reads' watermarks. Before any strap push, every NOOP value is null or empty, and freshness and coverage are `unknown`.
- **Bounds**: at most 90 entries and 512 KiB of JSON, with no raw streams, sleep sessions, stage segments or sync bookkeeping. The byte limit covers what the row caps cannot: free-text Workout, Exercise and NOOP sport names. A shorter range reads a block that exceeds it.

## Metric trends

`GET /api/training/trends/{metric}/{from_day}/{to_day}` with `Authorization: Bearer <API_KEY>`. MCP tool: `metric_trend(metric, from_day, to_day)`.

`metric` is exactly one of `body_weight`, `sleep_duration`, `resting_heart_rate`, or `hrv`. Dates are inclusive, canonical `YYYY-MM-DD` Copenhagen calendar days. The range must contain 1–90 days. Invalid metrics, dates, reversed ranges, and longer ranges return 400; missing or wrong credentials return 401. There are at most 90 daily points and 14 weekly summaries, so no measurement stream is returned or silently truncated.

The response has `metric`, `from_day`, `to_day`, `time_zone`, `unit`, `installation_id`, `last_push_at`, `daily_points`, and `weekly_summaries`. `installation_id` and `last_push_at` are null for Body Weight or before a NOOP push. A daily point is `{day, value, unit, source, freshness, coverage}`. Body Weight is read **only** from the daily Body Weight log; its source is `body_weight` when observed and its freshness and coverage are null because it is entered directly. NOOP metrics use the same imported-first, computed-fallback daily values as a Recovery Day. An edited Sleep Night can select the computed sleep block. The source identifies the selected device namespace; it is null for a missing value. Sleep duration is minutes, resting heart rate is beats/min, HRV is nightly RMSSD in milliseconds, and Body Weight is kg.

For NOOP points, `coverage` is `covered` only when applied windows cover all required rows in both strap namespaces; otherwise it is `unknown`. Sleep duration requires daily metrics and Sleep Night sessions; resting heart rate and HRV require daily metrics. `freshness` is `confirmed`, `partial`, `unconfirmed`, or `unknown` for those same required rows. A null with unknown coverage may mean the phone has not pushed that day; neither null nor unknown is converted to zero.

Each weekly summary has `week_start` (Monday), `week_end` (Sunday), `range_start`, `range_end`, `calendar_days`, `observed_days`, `mean`, and `unit`. Weeks use ISO Monday–Sunday boundaries in Europe/Copenhagen. `range_start` and `range_end` show where the requested range cuts a week. The arithmetic mean includes only days with a value; it is null when `observed_days` is zero. Counts expose sparse weeks and the response always includes the partial first and last weeks.

For example, a request for September 29 through October 2 with Body Weight logged on September 29 (80 kg) and October 1 (79 kg) has four daily points, two null values, and one weekly summary: `week_start: "2026-09-28"`, `week_end: "2026-10-04"`, `calendar_days: 4`, `observed_days: 2`, `mean: 79.5`.
