# AGENTS.md - Guidelines for AI Coding Agents

Rust axum API for a homelab service that serves RSS feeds and a timeline, protected by an API key.

## Project Scope

This file applies to the `api/` crate only. See root `AGENTS.md` for repo-wide guidelines.

## Runtime & Data Notes

- HTTP server binds to `0.0.0.0:8000`.
- SQLite database is configured via `DATABASE_URL`.
- Migrations live in `migrations/` and are applied on startup.
- API auth expects `Authorization: Bearer <API_KEY>`.
- NOOP push data lives in a separate SQLite file (`noop.db`, defaulting to the sibling of `DATABASE_URL`, overridable via `NOOP_DB_URL`) with its own pool and its own migrations in `noop_migrations/`. The split gives writer-lock isolation from `app.db`, lets the push store be backed up on its own (it is the only copy of received health data outside the phone), and gives it an independent rotation/lifecycle. Create push migrations with `just noop-migration-add <name>` (`sqlx migrate add -r --source noop_migrations`) and apply them with `just noop-migrate` (`sqlx migrate run` against `NOOP_DB_URL`); they are also applied on startup. Push-DB queries use runtime `sqlx::query` because the compile-time macros only check against `DATABASE_URL`.
- The push route authenticates with `Authorization: Bearer <NOOP_PUSH_TOKEN>` (required; must differ from `API_KEY`), and is merged outside the API-key middleware.
- Feed refresh runs on startup and every 24 hours in the background.

## Routes

- Health: `/api/health`
- Feeds: `/api/feeds`, `/api/feeds/{id}`, `/api/feeds/refresh`
- Timeline: `/api/timeline`, `/api/items/{id}/read`
- Exercises (fitness): `/api/exercises`, `/api/exercises/{id}` — list supports `?category=` and `?muscle_group=` filters; POST with a duplicate name returns 409; PATCH: absent fields keep the current value, explicit `null` clears the field (`muscle_group`, `equipment`); NOT NULL fields (`name`, `category`) cannot be cleared; DELETE returns 409 when the exercise is referenced by logged workouts
- Workouts (fitness): `/api/workouts` (GET/POST), `/api/workouts/{id}` (GET/PATCH/DELETE) — POST/PATCH create or replace the nested exercise entries and sets in one transaction (a failure rolls back the whole write); history list supports `?from=`, `?to=`, `?limit=` and is newest-first; PATCH: absent fields keep the current value, explicit `null` clears the field (`name`, `notes`, `body_weight_kg`); `date` is NOT NULL and cannot be cleared; DELETE cascades to entries and sets
- Progress (fitness): `/api/exercises/{id}/progress` — one row per workout date: `best_reps`, `best_weight_kg` (added weight), `total_volume_kg`, `best_rpe`, and `estimated_1rm_kg` (Epley: `weight * (1 + reps / 30)` from the best set of the day among sets with both reps and weight); observed data only, no interpolation; supports `?from=`, `?to=`, chronological order; unknown exercise is 404, a never-performed exercise returns an empty series
- Body weight (fitness): `/api/body_weight` (GET/POST) — one row per date (`date` is unique); list supports `?from=`, `?to=`, `?limit=` and is chronological; POST with a duplicate date returns 409; POST returns 201
- NOOP push (fixed route, not configurable): `/api/noop/push` — GET returns the capabilities document `{"type":"capabilities","protocolVersion","receiverStateId","streams"}` per `.noop/PUSH_PROTOCOL.md`; `NOOP-Push-Accept-Version` is checked first (no common version → 406), then the push token (→ 401); `receiverStateId` is persisted in `noop.db` (`receiver_state` table) and read per request, so an operator rotation takes effect immediately; unknown `/api/noop/*` paths → 404; errors use `{"type":"error","protocolVersion","code"}`
- Profile (fitness): `/api/profile` (GET/PATCH) — the single profile row; GET returns null fields when not configured; PATCH upserts (creates the row on first PATCH): absent fields keep the current value, explicit `null` clears the field (`height_cm`, `sex`)

## Workout logging conventions

- `weight_kg` means *added* weight: a bodyweight pull-up stores `weight_kg` null, a 40 kg weighted pull-up stores 40.
- A timed hold (Planche Hold, Handstand) stores `duration_seconds` with `reps` null; a set must have `reps` or `duration_seconds`, never neither.
- RPE is an integer 1–10 (null allowed).
- Volume for a set = `reps * weight_kg`, or `duration_seconds * weight_kg` when both are present; sets without added weight contribute zero.

## Project Structure

```
api/
├── migrations/              # SQLx migrations (app.db)
├── noop_migrations/         # SQLx migrations for the NOOP push store (noop.db)
└── src/
    ├── auth.rs              # API key auth middleware
    ├── db.rs                # Shared SQLite pool setup (app.db and noop.db)
    ├── error.rs             # AppError and JSON error response
    ├── lib.rs               # AppState and module wiring
    ├── main.rs              # Server bootstrap and scheduler
    ├── routes/              # HTTP route handlers
    │   ├── feeds.rs
    │   ├── fitness.rs
    │   ├── health.rs
    │   ├── noop_push.rs     # NOOP push endpoint (own bearer token)
    │   ├── timeline.rs
    │   └── mod.rs
    └── services/            # Domain services
        ├── feed.rs
        ├── noop_push.rs     # push protocol constants, config, receiver state
        └── mod.rs
```

## Available Skills

- `axum` - Expert guide for building production-ready web APIs with Axum 0.8+

## SQLx Usage

- Use `query!`/`query_as!` for compile-time checked SQL.
- Keep SQL strings in handlers/services; avoid string concatenation.
- Map unique violations to conflicts rather than internal errors.

## Logging

- Use `tracing` with structured fields.
- Prefer `info` for lifecycle events and `warn` for recoverable failures.

## Key Takeaways

1. Format and lint before committing.
2. Keep errors explicit, JSON-shaped, and safe.
3. Validate all inputs and reject unsafe URLs.
4. Prefer SQLx compile-time query macros.
5. Keep handlers thin; push logic to services.
