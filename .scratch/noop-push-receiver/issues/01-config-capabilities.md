# 01: Push receiver config + capabilities endpoint

**What to build:** The push path on the Axum API answers the NOOP client's capability discovery. A GET on the fixed push route with a valid bearer token and `NOOP-Push-Accept-Version` header returns the capabilities document: the negotiated `protocolVersion`, a stable persisted `receiverStateId`, and the advertised stream list (all 12 v1 registry streams). Wrong/missing version header → 406 before anything else; wrong or missing token → 401; unknown path → 404.

**Blocked by:** None (can start immediately).

**Status:** ready-for-agent

**Reference (read in this order):** `.noop/PUSH_PROTOCOL.md` — the wire contract, "Transport and authentication" + "Versioning" sections are the spec for this ticket. Only open `.noop/tests/PushEndpointPolicyTest.kt` and `SelfHostedPushSettingsTest.kt` if a behavior detail is ambiguous. Do not read the other reference files for this ticket.

- [x] `NOOP_PUSH_TOKEN` env var configures the push bearer token (the only required config; not reused from `API_KEY` since the phone client shouldn't hold the main key)
- [x] Push endpoint path is a single fixed route (e.g. `/api/noop/push`), documented — no path env var
- [x] Push data lives in a separate SQLite file, defaulted to the sibling of `DATABASE_URL` with the filename swapped to `noop.db` (own sqlx pool + own migrations dir), overridable via `NOOP_DB_URL` — decision recorded: split for writer-lock isolation, backup asymmetry (received data is the only copy outside the phone), and independent rotation/lifecycle
- [x] Push schema migrations are managed with sqlx-cli: files created with `sqlx migrate add -r` in the push migrations dir and applied with `sqlx migrate run` against `NOOP_DB_URL` (sqlx-cli is already in the devenv)
- [x] GET on the fixed push route with valid auth returns `{"type":"capabilities","protocolVersion":"1.0","receiverStateId":<uuid>,"streams":[all 12 names]}`, exactly matching the contract's required members
- [x] Version negotiation: first supported version in `NOOP-Push-Accept-Version` order is returned; no common version → 406 with a bounded `{"type":"error",...}` body
- [x] Missing/invalid bearer token → 401; token compared per request, never echoed
- [x] Capability JSON stays under 16 KiB and contains no members beyond the contract
- [x] Tests cover negotiation success/failure and auth failures
