# Workout logging and review (#121)

Public captures use fictional workouts in the seeded exercise library, served by
the production authenticated Axum fitness routes over an isolated migrated
in-memory database. Start holds the synthetic API credential on the server.

| Capture                       | Evidence                                                                             |
| ----------------------------- | ------------------------------------------------------------------------------------ |
| [Before](before.png)          | Workout placeholder at baseline `097c1ef`, 1180 × 820                                |
| [Landscape](landscape.png)    | History beside the selected workout with reps, added weight, holds and RPE           |
| [Exercise picker](picker.png) | Library search with category filter, grouped by muscle group                         |
| [Edit](edit.png)              | Whole-workout editor with reps/hold sets, bodyweight placeholder and RPE, 1180 × 820 |
| [Portrait](portrait.png)      | Editor sets stacked as type/removal over value, weight and RPE, 820 × 1180           |

The browser logged a workout through the dashboard and Axum stored a `0:22` hold
as 22 seconds with null reps, `12,5` as 12.5 kg added weight, an empty weight as
null bodyweight and the chosen RPE. Neither layout overflowed horizontally;
visible controls measured at least 44 px high.

The automated rendered seam runs against the real Axum fixture and verifies
logging reps, added weight, RPE and a bodyweight hold, then reviewing them from
history; field problems shown beside the set without saving; an edit drafting
from a fresh read after another device changed the workout; a rejected edit
(exercise removed from the library elsewhere) keeping both the draft and the
stored workout, with the failure beside the save button, before saving once
corrected with untouched set notes, durations and exact weights preserved;
removal only after confirmation; and an unreachable read rendering as
unavailable rather than an empty history.

Logged workouts stay separate from NOOP workouts; nothing matches or merges
them. `devenv shell -- just dashboard-check` passes. No iPad was attached, so the
physical touch check on iPad Safari remains part of release #69; these desktop
captures do not substitute for that hardware verification.
