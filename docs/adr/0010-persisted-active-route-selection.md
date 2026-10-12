# Active Route selection is an Axum snapshot

Axum remembers the measured loops it offers and accepts a generated identity
at `PUT /api/routes/active`. Clients cannot submit geometry, distances or target
metadata for selection. `GET /api/routes/active` returns `none` or the selected
snapshot, including its activity, requested target, stride, measured geometry,
distance and estimated steps. Both endpoints use the existing bearer API Key
and `Cache-Control: no-store`.

The candidate identity is derived from the activity, requested target, stride
and geometry with a versioned hash namespace. Cached generation has a stable
identity; the same geometry requested with a different effort has a different
identity and cannot overwrite an earlier selection's target. A singleton
SQLite selection increments its version atomically when the identity changes;
retrying the current identity retains its version. Selection and retrieval
never contact openrouteservice and remain usable after an API restart or
provider outage. Generation retains the 96 most recently offered candidates
plus the active candidate. Reoffering cached geometry refreshes retention;
a pruned identity returns a conflict and guidance to plan again.

We inspected the existing LoopPlanner, AppState SQLite pool and Wishlist and
Watchlist persistence. LoopPlanner owns provider search, its gate and cache;
selection has a different lifecycle and must outlive them. List persistence
has unrelated ownership and receipt rules. A focused `active_route` module
uses the existing pool, migration mechanism and checked SQLx queries rather
than adding another store or service abstraction. A short transaction keeps
the snapshot read and selected revision consistent under concurrent writes.

The Dashboard separates comparison from selection and restores the Active
Route on a fresh Travel load. Failed writes retain the last confirmed route;
a failed read is unavailable, never an empty selection. An ambiguous write
can be reconciled with Check selection. TanStack Start keeps the bearer key
on the server and reads map origin and selection concurrently.

An Active Route is the household's currently selected plan, not a started
Route Session. A phone may pin the returned identity/version and snapshot for
its own session. Replacing the Active Route must not silently replace a route
in a running session; that policy and phone GPS/handoff belong to #77.
