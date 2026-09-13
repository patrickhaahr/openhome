# AGENTS.md - Guidelines for AI Coding Agents

Rust axum API for a homelab service that serves RSS feeds and a timeline, protected by an API key.

## Project Scope

This file applies to the `api/` crate only. See root `AGENTS.md` for repo-wide guidelines.

## Runtime & Data Notes

- HTTP server binds to `0.0.0.0:8000`.
- SQLite database is configured via `DATABASE_URL`.
- Migrations live in `migrations/` and are applied on startup.
- API auth expects `Authorization: Bearer <API_KEY>`.
- Feed refresh runs on startup and every 24 hours in the background.

## Routes

- Health: `/api/health`
- Feeds: `/api/feeds`, `/api/feeds/{id}`, `/api/feeds/refresh`
- Timeline: `/api/timeline`, `/api/items/{id}/read`
- Exercises (fitness): `/api/exercises`, `/api/exercises/{id}` — list supports `?category=` and `?muscle_group=` filters; POST with a duplicate name returns 409; PATCH: absent fields keep the current value, explicit `null` clears the field (`muscle_group`, `equipment`); NOT NULL fields (`name`, `category`) cannot be cleared; DELETE returns 409 when the exercise is referenced by logged workouts
- Workouts (fitness): `/api/workouts` (GET/POST), `/api/workouts/{id}` (GET/PATCH/DELETE) — POST/PATCH create or replace the nested exercise entries and sets in one transaction (a failure rolls back the whole write); history list supports `?from=`, `?to=`, `?limit=` and is newest-first; PATCH: absent fields keep the current value, explicit `null` clears the field (`name`, `notes`, `body_weight_kg`); `date` is NOT NULL and cannot be cleared; DELETE cascades to entries and sets
- Progress (fitness): `/api/exercises/{id}/progress` — one row per workout date: `best_reps`, `best_weight_kg` (added weight), `total_volume_kg`, `best_rpe`, and `estimated_1rm_kg` (Epley: `weight * (1 + reps / 30)` from the best set of the day among sets with both reps and weight); observed data only, no interpolation; supports `?from=`, `?to=`, chronological order; unknown exercise is 404, a never-performed exercise returns an empty series
- Body weight (fitness): `/api/body_weight` (GET/POST) — one row per date (`date` is unique); list supports `?from=`, `?to=`, `?limit=` and is chronological; POST with a duplicate date returns 409; POST returns 201
- Profile (fitness): `/api/profile` (GET/PATCH) — the single profile row; GET returns null fields when not configured; PATCH upserts (creates the row on first PATCH): absent fields keep the current value, explicit `null` clears the field (`height_cm`, `sex`)

## Workout logging conventions

- `weight_kg` means *added* weight: a bodyweight pull-up stores `weight_kg` null, a 40 kg weighted pull-up stores 40.
- A timed hold (Planche Hold, Handstand) stores `duration_seconds` with `reps` null; a set must have `reps` or `duration_seconds`, never neither.
- RPE is an integer 1–10 (null allowed).
- Volume for a set = `reps * weight_kg`, or `duration_seconds * weight_kg` when both are present; sets without added weight contribute zero.

## Project Structure

```
api/
├── migrations/              # SQLx migrations
└── src/
    ├── auth.rs              # API key auth middleware
    ├── error.rs             # AppError and JSON error response
    ├── lib.rs               # AppState and module wiring
    ├── main.rs              # Server bootstrap and scheduler
    ├── routes/              # HTTP route handlers
    │   ├── feeds.rs
    │   ├── fitness.rs
    │   ├── health.rs
    │   ├── timeline.rs
    │   └── mod.rs
    └── services/            # Domain services
        ├── feed.rs
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
