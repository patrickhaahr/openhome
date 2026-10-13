# Combined exercise progress

`GET /api/exercises/progress?from=YYYY-MM-DD&to=YYYY-MM-DD[&exercise_ids=1,2,3]` with
`Authorization: Bearer <API_KEY>`. Issue
[#124](https://github.com/patrickhaahr/openhome/issues/124) and
[#125](https://github.com/patrickhaahr/openhome/issues/125), beneath
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
| Reps, with any positive Added Weight | `estimated_1rm` (`kg`): the best eligible added-load estimate on each date; unweighted sets do not contribute |
| Only Timed Holds, with any Added Weight | `unsupported`, `weighted_holds` |
| Reps and Timed Holds | `unsupported`, `mixed_set_kinds` |
| Weighted reps but no eligible finite estimate | `unsupported`, `no_valid_estimate` |
| Every value zero | `unsupported`, `no_positive_value` |
| None in the period, some earlier or later | `not_logged_in_range` |
| None ever | `never_logged` |

The best Set rather than total reps keeps the line about capability: adding a set does
not look like getting stronger. Kilogram-reps and kilogram-seconds are never combined.

### Weighted estimate and selection policy

Use Epley, `added_weight_kg × (1 + reps / 30)`, for 2–10 repetitions with a
positive load. One repetition uses the lifted load directly: a recorded single
should not be inflated by 3.3%. Zero repetitions and 11 or more repetitions are
excluded rather than extrapolated. Choose the greatest eligible estimate per day
across every Set and Workout; it need not be the heaviest Set.

The range is a conservative product choice: [Reynolds et al. (2006)](https://pubmed.ncbi.nlm.nih.gov/16937972/)
found 5RM more accurate than 10RM or 20RM for their bench/leg press sample.
These findings support limiting
extrapolation, not a claim that one equation is accurate for every movement.
[UKSCA's Epley/Brzycki validation study](https://cdn.uksca.org.uk/assets/pdfs/UkscaIqPdfs/2019-uksca-conference-posters-636981769331860781.pdf)
examined 5RM and 10RM loads. Epley keeps the chart familiar beside the existing
Progress Series, whose formula and unrestricted rep semantics remain unchanged.

These are estimates from recorded sets, not measured maximums. Predictions assume
near-maximal effort; ordinary submaximal training sets may underestimate capability.
RPE is not used to invent unperformed reps. `weight_kg` retains **Added Weight**
semantics: Body Weight is never added or inferred. In particular, the index for a
weighted bodyweight movement is an **added-load proxy**, not a validated estimate
of total strength. Its raw kilograms and this limitation are shown in the UI.

Metric selection uses all Sets in the requested period, regardless of Exercise
Category. Mixed reps and holds always take precedence and are unsupported. For
reps mixed between weighted and unweighted work, use the weighted metric throughout;
dates with only unweighted Sets or invalid estimates have no sample. `omitted_dates`
lists these logged dates explicitly, including nonfinite estimates/indexes. If every
weighted estimate is invalid, the Exercise has no line. Changing the period can
change the metric and baseline because it changes the observations being compared.

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
      ],
      "omitted_dates": []
    },
    {
      "exercise": { "id": 13, "name": "Converging Chest Press", "category": "gym" },
      "status": "charted",
      "metric": "estimated_1rm",
      "unit": "kg",
      "baseline": { "date": "2026-09-02", "value": 80.0 },
      "samples": [
        { "date": "2026-09-02", "value": 80.0, "index": 100.0 },
        { "date": "2026-09-22", "value": 60.0, "index": 75.0 }
      ],
      "omitted_dates": ["2026-09-04"]
    }
  ]
}
```

The response is bounded by 24 Exercises × 366 dates.
