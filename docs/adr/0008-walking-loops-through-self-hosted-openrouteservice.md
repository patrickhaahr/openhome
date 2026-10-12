# Walking loops are planned by Axum through self-hosted openrouteservice

The dashboard needs real walking loops from the private origin. Existing
integrations were checked: weather, IR, SwitchBot, AdGuard and Docker own no
routing, and the shared map only owns its origin and rendering. A dedicated
LoopPlanner owns the provider adapter, response validation and measurement. Its
route owns this state without a database dependency; selection and persistence
arrive with the Active Route.

Loops start at the validated MapOrigin, so they draw where the shared map opens
and need no second private coordinate. `POST /api/routes/loops` accepts an
activity and a whole-metre target. Only walking is accepted until running has
its own profile choice ([ADR 0009](0009-loop-alternatives-and-step-targets.md)
adds running, Step Targets and alternatives). Invalid requests are rejected before the provider is
contacted. Outcomes are values: missing origin or provider, provider outage or
refusal or invalid response, no route, and unsuitable route. Responses carry no
upstream messages. Logs carry only the HTTP status, the provider's numeric
error code and the transport error kind, never the origin-bearing request.

[openrouteservice](https://github.com/GIScience/openrouteservice) runs on the
household's own cluster rather than through HeiGIT's public API. Every request
contains the private origin, and the public API would also see the household's
IP address; self-hosting keeps both inside the household, needs no key or paid
plan, and removes third-party quotas. `OPENROUTESERVICE_URL` names the
instance's base URL (ending before `/v2`); Axum validates it at startup and
calls its `foot-walking` round-trip directions. The instance builds its graph
from OpenStreetMap data, so the dashboard attributes openrouteservice and
OpenStreetMap contributors beside generated loops. Requests are serialized and
time out after 15 seconds so repeated taps cannot pile work onto the instance.

The `foot-walking` profile already prefers footways, paths and pedestrian
ways. Its green and quiet weightings were not used: they need external green
and noise datasets that a standard self-hosted build does not include, and
sending them unsupported would fail every request. Requests avoid ferries.

The provider's length is only a preference, so Axum measures the returned
geometry itself and reports that distance and its deviation from the target.
A loop within ±10% matches the target; a loop up to ±50% away is shown and
labeled as a deviation; anything further is unsuitable rather than shown. A
loop must start within 400 m of the origin (openrouteservice snaps to ways
within 350 m by default) and end within 50 m of its start. The provider's own
summary distance must agree with the measured geometry within 10%, or the
response is treated as invalid. Measured distances describe what is drawn,
never what was requested. Provider error codes 2009 and 2010 mean no walkable
loop exists; every other refusal is a provider failure.

The round-trip seed is fixed, so the same origin and target plan the same loop.
The dashboard therefore stops offering to re-plan once a loop is shown, rather
than repeating identical work; varied alternatives arrive with distance and
step targets. [ADR 0009](0009-loop-alternatives-and-step-targets.md) replaces
the single seed with a bounded, cached search over several seeds.
