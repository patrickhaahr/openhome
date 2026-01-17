# AGENTS.md - Guidelines for AI Coding Agents

Rust axum API for a homelab service that serves facts, RSS feeds, and a timeline, protected by an API key.

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
- Facts: `/api/facts/random`
- Feeds: `/api/feeds`, `/api/feeds/{id}`, `/api/feeds/refresh`
- Timeline: `/api/timeline`, `/api/items/{id}/read`

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
    │   ├── facts.rs
    │   ├── feeds.rs
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
