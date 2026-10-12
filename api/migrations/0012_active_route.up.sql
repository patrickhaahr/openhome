-- Only Axum-generated measured loops are offered for selection.
CREATE TABLE route_candidates (
    id TEXT PRIMARY KEY NOT NULL,
    snapshot TEXT NOT NULL CHECK (json_valid(snapshot))
);
CREATE TABLE active_route (
    slot INTEGER PRIMARY KEY CHECK (slot = 1),
    route_id TEXT NOT NULL REFERENCES route_candidates(id),
    version INTEGER NOT NULL CHECK (version > 0 AND version <= 9007199254740991)
);
