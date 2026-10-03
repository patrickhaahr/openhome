# AGENTS.md - Guidelines for AI Coding Agents

Rust axum API for a homelab service that serves RSS feeds and a timeline, protected by an API key.

## Project Scope

This file applies to the `api/` crate only. See root `AGENTS.md` for repo-wide guidelines.

## Runtime & Data Notes

- HTTP server binds to `0.0.0.0:8000`.
- SQLite database is configured via `DATABASE_URL`.
- Migrations live in `migrations/` and are applied on startup.
- API auth expects `Authorization: Bearer <API_KEY>`.
- NOOP push data lives in a separate SQLite file (`noop.db`, defaulting to the sibling of `DATABASE_URL`, overridable via `NOOP_DB_URL`) with its own pool and its own migrations in `noop_migrations/`. The split gives writer-lock isolation from `app.db`, lets the push store be backed up on its own (it is the only copy of received health data outside the phone), and gives it an independent rotation/lifecycle. Create push migrations with `just api-noop-migration-add <name>` (`sqlx migrate add -r --source noop_migrations`) and apply them with `just api-noop-migrate` (`sqlx migrate run` against `NOOP_DB_URL`); they are also applied on startup. Push-DB queries use runtime `sqlx::query` because the compile-time macros only check against `DATABASE_URL`. Back up the whole `noop.db` file while the API is stopped, or use SQLite's online backup mechanism; `receiverStateId`, health rows, accepted batch acks, and incomplete replacement parts must be restored together.
- The push route is fixed at `/api/noop/push` on the same host and port as the other API routes. It authenticates with `Authorization: Bearer <NOOP_PUSH_TOKEN>` (required; must differ from `API_KEY`), and is merged outside the API-key middleware. In Compose, `./data` is mounted at `/app/data`; keep `NOOP_DB_URL` (if overridden) inside a persistent mount so restarts retain receiver state and retry history.
- Feed refresh runs on startup and every 24 hours in the background.

## Routes

- Health: `/api/health`
- Feeds: `/api/feeds`, `/api/feeds/{id}`, `/api/feeds/refresh`
- Timeline: `/api/timeline`, `/api/items/{id}/read`
- Exercises (fitness): `/api/exercises`, `/api/exercises/{id}` — list supports `?category=` and `?muscle_group=` filters; POST with a duplicate name returns 409; PATCH: absent fields keep the current value, explicit `null` clears the field (`muscle_group`, `equipment`); NOT NULL fields (`name`, `category`) cannot be cleared; DELETE returns 409 when the exercise is referenced by logged workouts
- Workouts (fitness): `/api/workouts` (GET/POST), `/api/workouts/{id}` (GET/PATCH/DELETE) — POST/PATCH create or replace the nested exercise entries and sets in one transaction (a failure rolls back the whole write); history list supports `?from=`, `?to=`, `?limit=` and is newest-first; PATCH: absent fields keep the current value, explicit `null` clears the field (`name`, `notes`); `date` is NOT NULL and cannot be cleared; DELETE cascades to entries and sets
- Progress (fitness): `/api/exercises/{id}/progress` — one row per workout date: `best_reps`, `best_weight_kg` (added weight), `total_volume_kg`, `best_rpe`, and `estimated_1rm_kg` (Epley: `weight * (1 + reps / 30)` from the best set of the day among sets with both reps and weight); observed data only, no interpolation; supports `?from=`, `?to=`, chronological order; unknown exercise is 404, a never-performed exercise returns an empty series
- Body weight (fitness): `/api/body_weight` (GET/POST) — one row per date (`date` is unique); list supports `?from=`, `?to=`, `?limit=` and is chronological; POST with a duplicate date returns 409; POST returns 201
- NOOP push (fixed route, not configurable): `/api/noop/push` — GET returns the capabilities document `{"type":"capabilities","protocolVersion","receiverStateId","streams"}` per `.noop/PUSH_PROTOCOL.md`; `NOOP-Push-Accept-Version` is checked first (no common version → 406), then the push token (→ 401); `receiverStateId` is persisted in `noop.db` (`receiver_state` table) and read per request, so an operator rotation takes effect immediately; unknown `/api/noop/*` paths → 404; errors use `{"type":"error","protocolVersion","code"}`
  - POST accepts one NDJSON batch (`Content-Type: application/x-ndjson`, identity or `gzip` Content-Encoding, anything else → 415). Push token checked first (→ 401), before the body is read. Bounds: 4 MiB + 64 KiB encoded, 4 MiB decoded (→ 413), 5,000 records. Append batches for the 8 append streams land in one typed STRICT table per stream (`hr_sample`, `rr_interval`, `event`, `battery`, `spo2_sample`, `skin_temp_sample`, `resp_sample`, `gravity_sample`), upserted by `(source_id, device_id, natural key)`; the registry (delivery mode, key/data columns, types, upsert/delete SQL) lives in `services/noop_push/registry.rs`. A stream's delivery mode is fixed by the registry (the other mode → 422 `invalid_header`)
  - Replace-window parts for the 4 mutable streams (`daily_metric`, `sleep_session`, `workout`, `journal` tables; `services/noop_push/replace_window.rs`): `window` is required, both cursors must be `null`, `recordCount` may be 0 only for a one-part (empty, still authoritative) window, and part numbering must satisfy `1 <= part <= parts`. Bounds are half-open and non-empty: canonical `YYYY-MM-DD` strings for the `day` selector (`dailyMetric`, `journal`), integer Unix seconds for `startTs` (`sleepSession`, `workout`); every record's selector key must lie inside the window (→ 422 `record_outside_window`). Parts are staged in `replacement_part` (decoded entity, cleared once applied or superseded); the part that completes the set applies the replacement in the same transaction (upsert all records, then delete rows in `(source_id, device_id)` and the window whose keys are absent; rows outside the window are untouched) and its ack (`endCursor: null`) is only returned after that commits. A duplicate key across parts → 422 `duplicate_key` on the completing part, which stays unstaged. `replacement` pins each `replacementId`'s stream, window and `parts` (reuse with different metadata, or a part number reused with another `batchId` → 409 `replacement_conflict`); `replacement_scope` tracks the latest generation per `(source, device, stream)`. A part of any other replacement supersedes a still-staging current one, whose parts then → 409 `replacement_superseded`. A byte-identical retry of an accepted part replays its ack without changing rows or the current generation, including parts of earlier completed replacements; retries of superseded incomplete replacements remain 409
  - Status codes: 400 broken framing/JSON or body (`malformed_ndjson`, `record_count_mismatch`, `invalid_content_encoding`, `unreadable_body`); 413 `payload_too_large`; 415 `unsupported_media_type`, `unsupported_content_encoding`; 422 valid JSON that violates the contract (`unsupported_protocol_version`, `unsupported_stream`, `invalid_header`, `invalid_cursor`, `invalid_window`, `empty_batch`, `invalid_record`, `record_outside_window`, `duplicate_key`); 409 `batch_conflict`, `replacement_conflict`, `replacement_superseded`; 500 `storage_unavailable`, `internal_error`. Unknown header/record/data members are ignored; key objects must contain exactly the registry columns. Records carry no rowids, so the cursor check is `endCursor.rowId - (startCursor.rowId or 0) >= recordCount` with positive rowids; `keySha256` must be 64 lowercase hex and is echoed verbatim, never recomputed
  - `batch_ledger` stores the SHA-256 of the decoded entity and the ack per `(receiver_state_id, source_id, device_id, batch_id)`. A byte-identical retry (gzip or identity) replays the stored ack without touching rows; different bytes → 409. Updating `receiver_state.receiver_state_id` to a different value fires the `receiver_state_rotation` trigger, atomically deleting all rows from `batch_ledger`, `replacement`, `replacement_scope`, and `replacement_part` while retaining health records. Writing the same ID preserves protocol state; rolling back the rotation also rolls back metadata deletion. Decode/parse/hash runs on `spawn_blocking`. Each batch is applied in one `BEGIN IMMEDIATE` transaction
- Training context (read-only): `{day}` is one Europe/Copenhagen calendar day (`YYYY-MM-DD`, else 400). The Recovery Day, Workouts on a Training Day and NOOP Workout Detail reads return a `noop` block with the installation, `last_push_at`, freshness and coverage.
  - `/api/training/days/{day}/recovery` returns the Recovery Day from the NOOP mirror: the Sleep Night waking on that day, resting HR, HRV, and NOOP's derived scores, each with its unit and source namespace. A night with more than 24 session rows gets 422 rather than being truncated.
  - `/api/training/sleep/recent/{count}` returns 1–14 consecutive Copenhagen wake days, newest first, including explicit missing nights. Each entry has the Recovery Day's sleep and NOOP status. Invalid counts get 400; an oversized night gets 422.
  - `/api/training/days/{day}/workouts` returns the Workouts logged in `app.db` with that date (ordered Exercises and Sets) beside the NOOP Workouts starting that local day, never paired. More than 500 logged Set rows or 24 NOOP workout rows gets 422.
  - `/api/training/noop-workouts/{source}/{start}[/{sport}]` returns the NOOP Workout Detail of one merged NOOP Workout, selected by the `source`, RFC 3339 `start` and `sport` that Workouts on a Training Day lists (`start` matched as an instant, `sport` case-insensitively). Without `sport`, several workouts sharing `source` and `start` are a 409 naming their sports, never a guess. It repeats the summary values, adds the average pace, and computes heart rate from the strap's `my-whoop` samples in `[start, end)` whatever namespace recorded the workout, falling back to the stored row's average and max (`hr.basis`). It carries the Training Day's workout freshness and coverage. Malformed `start`, `source` or `sport` is 400; an unknown or deduplicated-away row is 404; a workout over 24 h or a Training Day over 24 NOOP workout rows is 422.
  - `/api/training/exercises/{exercise_name}/history/{from_day}/{to_day}` returns exact logged Sets for one case-insensitively resolved Exercise over 1–90 inclusive calendar days. Unknown names are 404, ambiguous names 409; more than 500 joined Set rows is 422. It does not read NOOP.
  - `/api/training/trends/{metric}/{from_day}/{to_day}` returns 1–90 inclusive daily points and Monday–Sunday summaries for `body_weight`, `sleep_duration`, `resting_heart_rate`, or `hrv`. Missing values remain null and weekly means count only observed days. Body Weight comes only from the daily log; NOOP points name their source, freshness, and coverage.
  - `/api/training/context/{from_day}/{to_day}` returns the Training Context: one entry per day over 1–90 inclusive days with per-entry summaries of logged Workouts, the separate NOOP Workouts starting that day, daily Body Weight, and the Recovery Day's sleep duration, resting HR and HRV, plus per-day recovery and workout freshness and coverage. More than 1,000 logged Exercise entries, more than 24 NOOP workout rows on one day, or a response over 512 KiB of JSON gets 422.
  - The contracts, source-resolution rules and inspection SQL are in `docs/training-context.md`. The Hermes MCP adapter (`mcp/`) exposes `training_context`, `recovery_on_day`, `sleep_recent`, `workouts_on_day`, `noop_workout_detail`, `exercise_history`, and `metric_trend`.
  - Rules live in `services/training/`. Keep all interpretation there, not in the adapter (ADR 0002).
- Profile (fitness): `/api/profile` (GET/PATCH) — the single profile row; GET returns null fields when not configured; PATCH upserts (creates the row on first PATCH): absent fields keep the current value, explicit `null` clears the field (`height_cm`, `sex`, `hr_max_bpm`); `hr_max_bpm` is the user-set Max Heart Rate, an integer 100–240 inclusive (else 400), never inferred

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
    │   ├── training.rs      # Training context reads (Recovery Day, Workouts on a Training Day, ...)
    │   └── mod.rs
    └── services/            # Domain services
        ├── feed.rs
        ├── noop_push.rs     # push protocol constants, config, receiver state
        ├── noop_push/
        │   ├── ingest.rs    # batch decoding, NDJSON framing/validation, ledger + upserts
        │   ├── registry.rs  # v1 stream registry (delivery, columns, types, upsert/delete SQL)
        │   └── replace_window.rs # replace-window staging, generations, atomic apply
        ├── training.rs      # training read model over the training log and the NOOP mirror
        ├── training/
        │   ├── calendar.rs  # Europe/Copenhagen calendar days, DST-aware bounds, ISO rendering
        │   ├── context.rs   # Training Context: compact per-day view of a 1-90 day block
        │   ├── noop_source.rs # active installation, strap HR samples, freshness, coverage
        │   ├── noop_merge.rs  # NOOP's precedence rules (daily values, sleep sessions, workouts)
    │   ├── recovery.rs  # Recovery Day contract and assembly
    │   ├── workouts.rs  # Workouts on a Training Day contract and assembly
    │   ├── noop_workout_detail.rs # NOOP Workout Detail: one NOOP Workout with strap heart rate
    │   └── exercise_history.rs # exact Set history for one Exercise
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
