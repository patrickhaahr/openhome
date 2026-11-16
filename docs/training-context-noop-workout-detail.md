# Training context: NOOP Workout Detail

## NOOP Workout Detail

`GET /api/training/noop-workouts/{source}/{start}/{sport}`, or `GET /api/training/noop-workouts/{source}/{start}`, with `Authorization: Bearer <API_KEY>`. MCP tool: `noop_workout_detail(source, start, sport?)`.

One NOOP Workout from [Workouts on a Training Day](training-context-workouts.md#workouts-on-a-training-day), opened by its `source`, `start` and `sport` exactly as that read lists them. There is no separate identifier. The endpoint is read-only. It draws on the NOOP mirror (`noop.db`), plus the Profile's Max Heart Rate for Heart Rate Zones and Heart Rate Recovery.

- **`source`** is the NOOP Workout's device namespace, for example `my-whoop` or `apple-health`.
- **`start`** is its RFC 3339 start. It is matched as an instant, to the second, so `2026-09-27T16:33:52+02:00` and `2026-09-27T14:33:52Z` open the same workout.
- **`sport`** is its sport as listed, matched case-insensitively (`running` opens a `Running` workout). NOOP keeps workouts of different sports that share a namespace and start, for example a Running and a Cycling row at the same instant, so `source` and `start` alone do not always name one workout. Without `sport`, the single matching workout is returned; when several match, the read is a 409 that names their sports, and nothing is guessed. NOOP merges overlapping rows whose sports differ only in case, so a case-insensitive `sport` names one workout; for non-overlapping rows that differ only in case, the exact spelling wins.
- **Selection** runs against the merged NOOP Workout list of the Training Day that `start` falls on, in the active installation, after NOOP's [cross-source deduplication](training-context-rules.md#source-resolution). A row that NOOP folds into another one, or drops as a detected shadow, cannot be opened.

| Status | Meaning |
| --- | --- |
| 200 | The detail below. |
| 400 | `start` is not RFC 3339, or `source` or `sport` is empty or contains control characters. |
| 401 | Missing or wrong API key. |
| 404 | No merged NOOP Workout with that `source`, `start` and (when given) `sport` in the active installation, including before any strap push. |
| 409 | `sport` was left out and more than one merged NOOP Workout has that `source` and `start`. The error names their sports. |
| 422 | The workout lasts more than 24 hours, or more than 24 NOOP workout rows start on its Training Day. The read is rejected, never truncated. |
| 500 | Storage error. |

### Response

This example is the reference run of 2026-09-27, with a Max Heart Rate of 193 on the Profile. It was started manually in NOOP, and its stored row says 160 / 183 beats/min. The strap's samples, as the NOOP app shows them, say 166 / 185, and its Heart Rate Recovery matches the app's. The series is abbreviated to its first two points here; the response includes all 183 non-empty buckets.

```json
{
  "start": "2026-09-27T16:33:52+02:00",
  "end": "2026-09-27T17:19:32+02:00",
  "sport": "Running",
  "origin": "manual",
  "source": "my-whoop",
  "duration_min": 45.6597,
  "distance_m": 7169.41850045851,
  "energy_kcal": 817.170682229156,
  "strain_score": 58.45,
  "steps": null,
  "has_route": false,
  "avg_pace_s_per_km": 382.12053039235946,
  "hr": { "basis": "strap_samples", "avg_bpm": 166.07591240875914, "min_bpm": 84.0, "max_bpm": 185.0 },
  "hr_series": { "bucket_s": 15, "points": [{ "t_s": 0, "bpm": 86.6 }, { "t_s": 15, "bpm": 85.4 }] },
  "hr_zones": {
    "hrmax_used": 193,
    "below_zone_min": 1.5666666666666667,
    "zones": [
      { "zone": 1, "lower_bpm": 97, "upper_bpm": 115, "minutes": 0.6166666666666667, "percent": 1.3983371126228268 },
      { "zone": 2, "lower_bpm": 116, "upper_bpm": 135, "minutes": 1.55, "percent": 3.5147392290249435 },
      { "zone": 3, "lower_bpm": 136, "upper_bpm": 154, "minutes": 2.783333333333333, "percent": 6.311413454270596 },
      { "zone": 4, "lower_bpm": 155, "upper_bpm": 173, "minutes": 17.2, "percent": 39.002267573696145 },
      { "zone": 5, "lower_bpm": 174, "upper_bpm": 193, "minutes": 21.95, "percent": 49.77324263038549 }
    ]
  },
  "hr_recovery": {
    "end_hr_bpm": 183,
    "at_1_min": { "hr_bpm": 152, "drop_bpm": 31 },
    "at_2_min": { "hr_bpm": 139, "drop_bpm": 44 },
    "at_5_min": { "hr_bpm": 134, "drop_bpm": 49 }
  },
  "unavailable": {},
  "noop": {
    "installation_id": "81906e30-187d-4546-8f8a-9949b82d62fa",
    "imported_device_id": "my-whoop",
    "computed_device_id": "my-whoop-noop",
    "last_push_at": "2026-09-28T06:00:00+02:00",
    "freshness": "confirmed",
    "coverage": { "workouts": "covered" }
  }
}
```

- **Summary**: `start`, `end`, `sport`, `origin`, `source`, `duration_min`, `distance_m`, `energy_kcal` and `strain_score` have the same values and meanings as in `noop_workouts`.
- **`steps`**: the steps counted over the workout; null when none were recorded. `noop_workouts` carries the same value.
- **`has_route`**: whether a usable GPS route was captured.
- **`route`**: the route as `[[latitude, longitude], ...]` in degrees, in order. It is only present when the request carries `?route=true` and a route exists; otherwise the key is absent.
  - NOOP stores the route as a Google Encoded Polyline with precision 5 (`RouteMath` in the NOOP app). The API decodes it, keeps what parses, drops points off the globe, and reports `has_route: false` when nothing usable remains.
  - A route longer than 300 points is thinned to 300 evenly by index, keeping the first and last point.
  - Coordinates are private household data: only authenticated clients receive them, they are never logged, and the MCP adapter does not ask for them.
- **`avg_pace_s_per_km`**: NOOP's recorded duration in seconds divided by the distance in km. It is null when the distance is null, zero or negative, or the duration is null.
- **`hr`**: heart rate in beats/min over the workout window `[start, end)`.
  - Heart rate always comes from the strap: samples in the strap's imported namespace `my-whoop` of the active installation. This holds whatever namespace recorded the NOOP Workout, because the strap is the user's only heart rate sensor.
  - `basis: "strap_samples"`: the window holds at least one strap sample. `avg_bpm`, `min_bpm` and `max_bpm` are computed over those samples, as the NOOP app does.
  - `basis: "workout_row"`: the window has no strap samples. `avg_bpm` and `max_bpm` are the stored row's values, which NOOP may not have reconciled with the strap. `min_bpm` is null.
  - `basis: null`: neither exists, and every value is null.
- **`hr_series`**: a bounded strap heart rate series over the same `[start, end)` window, with points in chronological order.
  - `bucket_s` is 15 seconds for workouts up to 4,500 seconds. For longer workouts it is the smallest multiple of 15 that keeps `ceil((end - start) / bucket_s)` at most 300, regardless of gaps or NOOP's recorded `duration_min`. A 24-hour workout uses 300-second buckets and has at most 288 points.
  - Each point is `{t_s, bpm}`: `t_s` is the bucket start's offset in seconds from the workout's `start`, and `bpm` is the arithmetic mean of the strap samples in that bucket. Buckets are half-open and anchored to the workout start, not wall-clock boundaries; the final bucket can be partial.
  - Empty buckets are omitted, never zero-filled or interpolated. There are at most 300 points. Only this series is downsampled; other detail values still use full-resolution samples.
  - When `hr.basis` is `workout_row` or null, `hr_series` is null and `unavailable.hr_series` is `"no_strap_samples"`. Stored average/max heart rate cannot substitute for a measured series.
- **`hr_zones`**: time in each Heart Rate Zone over the same `[start, end)` window, computed with the NOOP app's algorithm (`HRZones`), so the split is identical to the app's for the same run and the same Max Heart Rate.
  - `hrmax_used` is the Profile's current Max Heart Rate (`hr_max_bpm`, set with `PATCH /api/profile`). It must equal the NOOP app's manual override for the zones to match. It is never inferred, and there is no history: a changed value re-zones every workout on the next read, as the app re-zones its history.
  - Zones 1–5 have inclusive lower edges at 50 / 60 / 70 / 80 / 90 % of `hrmax_used`; Zone 5 is open-ended. A sample's zone is decided against the unrounded thresholds: at 193, 135 bpm is below the 135.1 bpm edge of Zone 3. `lower_bpm` is `ceil(pct × hrmax_used)`, the lowest whole bpm in the zone. `upper_bpm` is the next zone's `lower_bpm` − 1, and `hrmax_used` for Zone 5, which still counts higher samples.
  - **Time credit**: each sample is credited with the time until the next sample in the window, capped at the window's median sample interval, so a gap in the data never inflates one zone. The last sample is credited with the median interval. The median is the upper median of the gaps shorter than 300 s, at least 1 s, and 1 s when there is no such gap.
  - Time below 50 % goes to `below_zone_min` and is left out of the percentages. `percent` is the zone's share of the time in Zones 1–5, so the five sum to 100 (± float rounding), or are all 0 when no time is in a zone. Minutes and percentages are unrounded; the NOOP app shows them rounded (percentages) and floored (minutes).
  - Null when the Profile has no Max Heart Rate (`unavailable.hr_zones: "hr_max_not_configured"`, whether or not there are samples) or when the window has no strap samples (`"no_strap_samples"`). Stored zones on the workout row are never used.
- **`hr_recovery`**: Heart Rate Recovery, computed as the NOOP app computes it, from strap samples in `[start, end + 315 s]`. Samples outside 30–250 beats/min are ignored.
  - **Eligibility**: within the last 300 seconds of the workout, heart rate must stay at or above 70 % of the Profile's Max Heart Rate for at least 120 seconds continuously. Consecutive samples at most 10 seconds apart are continuous; a longer gap restarts the count. Recovery after an easy session or a long cool-down is not meaningful, so it is not reported.
  - **`end_hr_bpm`**: the highest heart rate in the workout's final 30 seconds (`[end - 30 s, end]`), which needs at least 3 samples. It is not the session peak.
  - **`at_1_min`, `at_2_min`, `at_5_min`**: `{hr_bpm, drop_bpm}` at `end` + 60, 120 and 300 seconds. `hr_bpm` is the median of the samples within ±15 seconds of the mark (the mean of the middle two, rounded half away from zero, for an even count), and `drop_bpm` is `end_hr_bpm - hr_bpm`. A mark with fewer than 3 samples, for example because the strap stopped or has not synced yet, is null; it is never interpolated.
  - `hr_recovery` is null, with the reason in `unavailable.hr_recovery`, when there is no Max Heart Rate on the Profile (`hr_max_not_configured`, checked first), the workout window has no strap samples (`no_strap_samples`), eligibility is not met (`not_sustained`), or the final 30 seconds have fewer than 3 samples (`no_end_samples`).
- **`unavailable`**: names why a null block of the detail is null, keyed by the block. A key is present only for a null block: `{}` when strap samples and a Max Heart Rate provide everything, `{"hr_zones": "hr_max_not_configured", "hr_recovery": "hr_max_not_configured"}` without a Max Heart Rate, or `{"hr_series": "no_strap_samples", "hr_zones": "no_strap_samples", "hr_recovery": "no_strap_samples"}` without strap samples. `hr_recovery` can also be the only null block (`not_sustained`, `no_end_samples`).
- **`noop`**: the same block as Workouts on the Training Day the workout starts on. The strap's heart rate stream is append-only and has no replacement windows, so the protocol gives no coverage signal for samples. Sample sufficiency shows in `hr.basis` instead.
- **Excluded data**: raw samples and RR intervals are never returned. The route is only returned by an explicit `?route=true` read of the detail, and coordinates are never logged.
