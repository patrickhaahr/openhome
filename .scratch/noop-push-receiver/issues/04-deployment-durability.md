# 04: Deployment wiring + restart durability

**What to build:** The receiver runs as part of the existing api deployment and survives restarts the way the contract assumes. Migrations for all push tables are applied on startup; `receiverStateId` stays stable across restarts and database backups; the batch ledger and staged replacement parts persist, so a client retrying after a receiver restart gets replayed acks (or 409s for conflicting bytes) rather than duplicate rows. Env-var configuration is documented alongside the existing `DATABASE_URL`/`API_KEY` pattern, and the push path is reachable through the same deployment that serves the other API routes.

**Blocked by:** 03 (all streams working).

**Status:** ready-for-agent

**Reference:** none required — this ticket is infrastructure work against the repo's own deployment patterns. `.noop/` exists only if a durability claim needs cross-checking.

- [x] A receiver restart preserves: stored records, batch ledger (replay + 409 semantics), staged replacements, and `receiverStateId`
- [x] Push migrations for `NOOP_DB_URL` run via `sqlx migrate run` (sqlx-cli); migrations verified idempotent across restarts
- [x] Env config documented in `api/AGENTS.md` runtime notes: required `NOOP_PUSH_TOKEN`, fixed push route, `NOOP_DB_URL` default derived from `DATABASE_URL`
- [x] A manual smoke check: restart the server mid-baseline and confirm the client retry converges without duplicates
