# Recovery and NOOP training summaries (#123)

Workout → Recovery & NOOP reads the existing authenticated Recovery Day,
Training Context and NOOP Workout Detail routes. The selected day ends a
seven-day context; every calendar day remains present. Axum owns source
selection, Copenhagen day alignment, coverage, freshness, zones and recovery.
Start keeps the API credential on the server and returns sanitized failures.

The public captures use only synthetic observations in the loopback fixture's
isolated, migrated databases. Its NOOP batches pass through the existing push
ingestion contract, including applied empty replacement windows. The sample
day is 2026-09-30; 2026-10-03 demonstrates unknown coverage. No household
identifiers, health records or credentials were captured.

- [Before](before.png): Workout at baseline `178d3a6`, without a recovery view.
- [Landscape](landscape.png): selected-day recovery and separate NOOP detail entry point, 1180 × 820.
- [Detail](detail.png): exact NOOP session, raw units, provenance and strap heart rate.
- [Portrait](portrait.png): selected detail stacks at 820 × 1180.
- [Context](context.png): logged Workouts beside separate NOOP Workouts, including two sports sharing a start.
- [Unknown coverage](unknown-coverage.png): null observations remain explicitly unsynced/unavailable.

Reproduce with `devenv shell -- just dashboard-health-fixture-run`, then
`devenv shell -- env OPENHOME_API_URL=<printed-loopback-url>
API_KEY=synthetic-dashboard-api-key just dashboard-run`. Open
`/workout?view=recovery&day=2026-09-30`. Set the synthetic Profile's maximum
heart rate to 190 in Profile & weight to inspect zones and Heart Rate Recovery.
The context capture additionally logs a fictional Workout through the normal
fitness API. The fixture does not pre-populate the training log or Profile.

The rendered tests use these production routes and cover selected sources,
units, sleep sessions/journal, independent logged/NOOP records, exact
source/start/sport identity (including `/` in a sport), missing strap samples,
Profile-dependent zones, recovery marks with missing observations, series gaps,
covered absence versus unsynced data, authorization recovery, independent read
failures, sanitized diagnostics and returning focus to the launching button.
Existing API training integration fixtures remain unchanged.

Browser checks found no horizontal overflow in either orientation; visible
training buttons measured 48 px high. Opening detail moves focus and scroll to
it; closing returns focus to the launching button. Axe reported no violations
in the overview or detail. Detail SVG text required manual contrast inspection;
the muted text/card tokens measured 8.06:1. This is Chromium viewport evidence,
not physical touch or iPad Safari evidence.

The configured NOOP source was confirmed privately with a read-only query of
the local mirror's current receiver state. No health values or installation
identifier were extracted into public evidence. This does not claim a new
phone sync or deployed end-to-end dashboard verification. USB mux was inactive
and no iPad was attached; real iPad Safari/touch verification remains an
integrated release gate under #69/#81.
