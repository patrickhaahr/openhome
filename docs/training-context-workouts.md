# Training context: workouts

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
  - A NOOP Workout's `source`, `start` and `sport` open its [NOOP Workout Detail](training-context-noop-workout-detail.md#noop-workout-detail), with heart rate from the strap's samples, Heart Rate Zones, Heart Rate Recovery and the average pace.
  - Stored HR zones and raw streams are never returned; routes only come with `?route=true` on the NOOP workout detail.
- **An empty `noop_workouts` never means the user did not train.** With `coverage.workouts: covered`, NOOP recorded no workout starting that day. With `unknown`, rows may not have been pushed yet.

## Exercise Set history

`GET /api/training/exercises/{exercise_name}/history/{from_day}/{to_day}` with `Authorization: Bearer <API_KEY>`. MCP tool: `exercise_history(exercise_name, from_day, to_day)`.

Both dates are inclusive, canonical `YYYY-MM-DD` Europe/Copenhagen calendar days. The interval must contain 1–90 days. The Exercise name is resolved case-insensitively against the Exercise library. If two names differ only by case, either spelling is ambiguous. The result reads only the OpenHome training log, so it has no NOOP freshness or coverage block.

| Status | Meaning |
| --- | --- |
| 200 | Exact dated Workouts and Sets. A known Exercise with no history returns `"workouts": []`. |
| 400 | Invalid date, reversed range, or more than 90 days. |
| 401 | Missing or wrong API key. |
| 404 | No Exercise matches the name. |
| 409 | More than one Exercise matches the name case-insensitively. |
| 422 | More than 500 joined Set rows, counting an Exercise entry without Sets as one. The result is rejected, never truncated. |

```json
{
  "exercise": {"id": 11, "name": "Pull-up", "category": "calisthenics"},
  "from_day": "2026-09-01",
  "to_day": "2026-09-26",
  "time_zone": "Europe/Copenhagen",
  "workouts": [
    {
      "id": 6, "date": "2026-09-26", "name": "Pull", "notes": null,
      "entries": [
        {"notes": "wide grip", "sets": [
          {"set_number": 1, "reps": 5, "added_weight_kg": 12.5,
           "hold_duration_s": null, "rpe": 10, "notes": "+2 partials"}
        ]}
      ]
    }
  ]
}
```

Workouts are ordered by date, then creation id. Repeated entries of the Exercise within one Workout remain separate and retain their logged order; an entry without Sets has `"sets": []`. Sets are ordered by `set_number`, then id. `added_weight_kg` is weight added to the body, null for a bodyweight Set. `hold_duration_s` is a Timed Hold. Optional values remain null. The API owns this grouping and validation; MCP returns its JSON unchanged.
