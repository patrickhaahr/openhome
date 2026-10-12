# Loop alternatives come from a bounded, cached seed search in Axum

The household chooses walking or running, a 5 km or 10 km preset, a custom
distance or a Step Target, and compares up to three Loop Alternatives. This
extends the LoopPlanner from [ADR 0008](0008-walking-loops-through-self-hosted-openrouteservice.md)
rather than adding an adapter: the provider, origin and measurement rules stay
the same, and the dashboard still only displays what Axum returns.

`POST /api/routes/loops` takes an activity, a target of
`{ "kind": "distance", "meters" }` or `{ "kind": "steps", "steps" }`, and a
whole-centimetre `step_length_cm` (40–150). Axum converts steps to metres with
that step length and applies the existing 1–25 km range to the result, so a
Step Target and a distance target share every later rule. Each Loop carries
`estimated_steps`, its measured distance divided by the step length; the
dashboard labels it as an estimate. The dashboard keeps a separate default
step length for walking (75 cm) and running (100 cm) and shows Axum's own
message when it rejects a target.

Running uses the same `foot-walking` graph and additionally avoids steps. A
standard self-hosted instance builds no running profile, and `foot-hiking`
favours trails over the streets and parks a daily run uses. This supersedes ADR
0008's walking-only rule.

openrouteservice returns one round trip per request, varied by its seed. Axum
requests seeds 1–6 in order and stops once three distinct loops fall within
±10% of the target. After the first loop, each request's length is corrected
by the mean overshoot so far (bounded to 60–140% of the target), because the
provider's length is only a preference. A loop is distinct when at most 70% of
points sampled every 100 m along it lie within 60 m of an already kept loop.
Loops within ±50% are kept; up to three are returned, matching loops first and
closest first. The plan reports how many matches were sought and whether the
search was `complete`, `exhausted` (all attempts spent) or `interrupted`, so
the dashboard can explain a shortfall without padding the set. Duplicates and
mismatches are never relabelled as matches.

The search is bounded for the household's instance. One search runs at a time;
it starts no attempt after 20 seconds and each attempt times out after 10. An
outage or invalid response ends the search: loops already kept are returned as
`interrupted`, otherwise the outcome is `provider-unavailable`. Provider code
2010 (no walkable way near the origin) ends it as `no-route`; 2009 only spends
that attempt. Seeds are fixed, so the same origin and target plan the same
loops: completed searches are cached in memory for 30 minutes (32 entries) by
activity and target metres, and a request queued behind an identical one reuses
its answer. Interrupted searches are not cached, so a retry tries again.

When a later request fails, the dashboard keeps showing the earlier Loop
Alternatives with their own target in the heading and says so beside the
failure. Selecting and persisting an Active Route remains separate.
