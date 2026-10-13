# Combined exercise progress

`GET /api/exercises/progress?from=YYYY-MM-DD&to=YYYY-MM-DD[&exercise_ids=1,2,3]` with
`Authorization: Bearer <API_KEY>`. Issue
[#124](https://github.com/patrickhaahr/openhome/issues/124), beneath
[#81](https://github.com/patrickhaahr/openhome/issues/81).

One bounded read returns a **Progress Line** for several Exercises, so the dashboard
chart needs no request per Exercise. The rules live in
`api/src/services/exercise_progress.rs`. `GET /api/exercises/{id}/progress` (the
Progress Series) is unchanged.

| Status | Meaning |
| --- | --- |
| 200 | One entry per selected Exercise. |
| 400 | Missing or non-canonical `from`/`to`, `from` after `to`, a period longer than 366 days, or `exercise_ids` that is empty, malformed, repeats an id or lists more than 24. |
| 401 | Missing or wrong API key. |
| 404 | An id in `exercise_ids` is not in the exercise library. |

## Selection

- With `exercise_ids`, entries follow the requested order.
- Without it, the Exercises with Sets in the period are selected, most recently logged
  first (then by name), up to 24. `logged_exercise_count` always counts every Exercise
  with Sets in the period, so a client can say when the default selection is partial.

## Metric

Each Exercise gets one metric from its Sets in the period. An Added Weight of zero
adds nothing, so that Set counts as bodyweight work.

| Sets in the period | Result |
| --- | --- |
| Only reps without Added Weight | `repetitions` (`reps`): the best Set's reps on each date |
| Only Timed Holds without Added Weight | `hold_seconds` (`s`): the longest hold on each date |
| Any Set with Added Weight | `unsupported`, `added_weight` (weighted lifts follow in #125) |
| Reps and Timed Holds | `unsupported`, `mixed_set_kinds` |
| Every value zero | `unsupported`, `no_positive_value` |
| None in the period, some earlier or later | `not_logged_in_range` |
| None ever | `never_logged` |

The best Set rather than total reps keeps the line about capability: adding a set does
not look like getting stronger. Kilogram-reps and kilogram-seconds are never combined.

## Index

The baseline is the first date in the period whose value is above zero. Every sample is
`value / baseline × 100`; an earlier zero is kept and indexes to 0. Samples exist only
on logged dates, so plateaus, declines and gaps stay visible.

```json
{
  "from": "2026-09-01",
  "to": "2026-09-30",
  "logged_exercise_count": 2,
  "exercises": [
    {
      "exercise": { "id": 11, "name": "Pull-up", "category": "calisthenics" },
      "status": "charted",
      "metric": "repetitions",
      "unit": "reps",
      "baseline": { "date": "2026-09-01", "value": 8 },
      "samples": [
        { "date": "2026-09-01", "value": 8, "index": 100.0 },
        { "date": "2026-09-03", "value": 10, "index": 125.0 },
        { "date": "2026-09-12", "value": 6, "index": 75.0 }
      ]
    },
    {
      "exercise": { "id": 13, "name": "Converging Chest Press", "category": "gym" },
      "status": "unsupported",
      "reason": "added_weight"
    }
  ]
}
```

The response is bounded by 24 Exercises × 366 dates.
