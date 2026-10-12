# Walking loop (#106)

Travel can now plan a 5 km walking loop from the shared map origin. Axum asks
the household's self-hosted openrouteservice for a pedestrian round trip,
measures the returned geometry itself and only then hands it to the dashboard.
The dashboard draws that geometry on the shared map and reports the measured
length against the target.

Configure the Axum API privately with `OPENROUTESERVICE_URL`, the instance base
URL ending before `/v2` (for example `http://openrouteservice:8082/ors`). The
instance must have the `foot-walking` profile enabled, and a map origin must be
configured. Leaving it empty shows setup instructions. OpenHome never calls
HeiGIT's public API: the origin goes only to the household's own instance.

The captures use the isolated authenticated Axum fixture with the unrelated
central Copenhagen demo point. **The drawn loop is a synthetic rectangle served
by a loopback stand-in for openrouteservice, not a real walkable path.** It
exercises the real request, measurement and rendering path end to end.

| Before (1024 × 768)         | After (1024 × 768)                  |
| --------------------------- | ----------------------------------- |
| ![Idle planner](before.png) | ![Measured 5.10 km loop](after.png) |

| 3D buildings (1024 × 768)               | Portrait (768 × 1024)                               |
| --------------------------------------- | --------------------------------------------------- |
| ![Loop over 3D buildings](after-3d.png) | ![Loop and planner in portrait](after-portrait.png) |

## Observed evidence — 2026-10-12

- Axum integration tests drive a wiremock openrouteservice through the
  authenticated route: request shape (`round_trip.length`, ferries avoided),
  a matching loop, a labeled 16% deviation, rejection of loops that start far
  from the origin, do not close or miss the target by more than 50%, malformed
  or disagreeing provider responses, no-route versus outage outcomes, and
  invalid requests rejected before the provider is contacted. Upstream error
  text never reaches the response or the logs.
- Dashboard component tests run against the real Axum fixture: a generated
  loop is measured and drawn from the origin, provider failures and malformed
  routes clear the map instead of keeping an old loop, and an out-of-tolerance
  loop is labeled as outside ±10%.
- Desktop preview: the loop is fitted once, stays drawn when switching to 3D
  buildings and reflows below the map in portrait.

## Pending: real self-hosted provider

The household's openrouteservice 10.0.1 instance is reachable, but so far it
builds only the `driving-car` graph. Against it, the real Axum route answers
`provider-unavailable` (openrouteservice code 2003, unknown profile) rather than
inventing a route. The same request against `driving-car` returned the exact
response shape the planner parses: one closed `LineString` starting 46 m from
the demo origin, with a 6330.9 m summary that agrees with the 6330.5 m measured
from its geometry. No pedestrian loop has been inspected yet.

Once the instance builds `foot-walking`, run the fixture against it to plan a
real loop from the demo origin and check that it uses walkable paths:

```sh
OPENROUTESERVICE_URL=https://<instance>/ors devenv shell -- just dashboard-health-fixture-run
```
