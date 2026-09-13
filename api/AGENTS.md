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
- Exercises (fitness): `/api/exercises`, `/api/exercises/{id}` — list supports `?category=` and `?muscle_group=` filters; POST with a duplicate name returns 409; PATCH treats absent and `null` fields identically (keeps the current value — optional fields cannot be unset via PATCH); DELETE returns 409 when the exercise is referenced by logged workouts
- Workouts (fitness): `/api/workouts` (GET/POST), `/api/workouts/{id}` (GET/PATCH/DELETE) — POST/PATCH create or replace the nested exercise entries and sets in one transaction (a failure rolls back the whole write); history list supports `?from=`, `?to=`, `?limit=` and is newest-first; PATCH treats absent and `null` workout fields identically (keeps the current value — optional fields cannot be unset via PATCH); DELETE cascades to entries and sets

## Workout logging conventions

- `weight_kg` means *added* weight: a bodyweight pull-up stores `weight_kg` null, a 40 kg weighted pull-up stores 40.
- A timed hold (Planche Hold, Handstand) stores `duration_seconds` with `reps` null; a set must have `reps` or `duration_seconds`, never neither.
- RPE is an integer 1–10 (null allowed).

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
