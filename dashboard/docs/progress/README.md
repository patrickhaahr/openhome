# Combined exercise progress (#124)

The Workout page's **Progress** view (`/workout?view=progress`) charts
[combined exercise progress](../../../docs/exercise-progress.md) from one
authenticated Axum read. Start keeps the API credential on the server.

Captures use synthetic workouts in the seeded exercise library, served by the
isolated `dashboard-health-fixture-run` API. No household data is shown.

| Capture                                       | Evidence                                                                                                                                        |
| --------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| [Landscape](landscape.png), 1024 × 768        | Five recent lines over three months, including a decline (Dip), plateaus and dashed joins across gaps longer than two weeks                     |
| [Portrait](portrait.png), 768 × 1024          | The same chart and controls stacked without horizontal overflow                                                                                 |
| [Compare](compare.png), 1024 × 768, full page | Two lines turned off, a never-logged exercise added, a weighted lift listed under Not charted, and Planche Hold's raw seconds beside each index |

- The period toggle offers 4 weeks, 3 months (default) and 12 months, each ending today.
- Without a choice, Axum selects the most recently logged exercises. **Add exercise**
  compares exactly the chosen ones; **Most recent** returns to the default.
- Each legend entry is a toggle for its line, with its measure, raw baseline and
  date, and the latest raw value and index. Six lines start shown. Colours follow
  the read order, and toggled lines keep their state when the period or compared
  exercises change.
- **Observed values** lists every logged date for one line, with its raw value,
  unit and index.
- Never-logged, out-of-period and unsupported exercises appear under **Not charted**
  with their reason; they get no line.
- An unreachable read shows _Progress unavailable_ rather than an empty chart.

The rendered seam (`tests/exercise-progress.test.tsx`) runs against the real Axum
fixture: indexes and raw values, line toggling, Not charted reasons, the
compare/period flow and the unavailable state. In the browser, no layout
overflowed horizontally and the visible controls were at least 44 px high.

No iPad was attached, so the physical touch, readability and chart-load check
on iPad Safari is still part of release #69. These desktop captures do not
replace that hardware check.
