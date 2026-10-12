# Walking loops are planned by Axum through openrouteservice

The dashboard needs real walking loops from the private origin. Existing
integrations were checked: weather, IR, SwitchBot, AdGuard and Docker own no
routing, and the shared map only owns its origin and rendering. A dedicated
LoopPlanner owns the provider adapter, response validation and measurement. Its
route owns this state without a database dependency; selection and persistence
arrive with the Active Route.

Loops start at the validated MapOrigin, so they draw where the shared map opens
and need no second private coordinate. `POST /api/routes/loops` accepts an
activity and a whole-metre target. Only walking is accepted until running has
its own profile choice. Invalid requests are rejected before the provider is
contacted. Outcomes are values: missing origin or provider, rejected key, quota,
provider outage or invalid response, no route, and unsuitable route. Neither
responses nor logs carry upstream messages, the provider key or request bodies.

[openrouteservice](https://openrouteservice.org/) by HeiGIT plans round trips
on its `foot-walking` profile with a target length. Requests weight green areas
fully and quiet ways moderately, and avoid ferries. Its free Standard plan,
checked 2026-10-12, allows 2,000 directions requests per day and 40 per minute
with a free key and no paid plan. Round trips are limited to 100 km. Requests
are serialized and time out after 15 seconds. The dashboard shows the
openrouteservice and OpenStreetMap attribution beside generated loops.

The provider's length is only a preference, so Axum measures the returned
geometry itself and reports that distance and its deviation from the target.
A loop within ±10% matches the target; a loop up to ±50% away is shown and
labeled as a deviation; anything further is unsuitable rather than shown. A
loop must start within 400 m of the origin (the provider snaps to ways within
350 m) and end within 50 m of its start. The provider's own summary distance
must agree with the measured geometry within 10%, or the response is treated as
invalid. Measured distances describe what is drawn, never what was requested.

The gateway answers a spent daily quota with 403, the same status as a rejected
key. Axum distinguishes them by the quota wording in the body; 429 is the
per-minute limit. Error codes 2009 and 2010 mean no walkable loop exists.
