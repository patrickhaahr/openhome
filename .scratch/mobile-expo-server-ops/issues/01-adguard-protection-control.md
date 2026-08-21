# 01 — AdGuard Protection control

**What to build:** The Server Tab exists with an AdGuard Protection card that shows the current protection state (protected / paused / unprotected), the remaining time on a Protection Pause, the AdGuard version and running indicator, and offers enable, disable, and pause for 5/15/30/60 minutes with retry when status cannot be fetched. Establishes the Server Tab shell plus the pull-to-refresh and foreground-refocus refetch convention reused by later server-operations tickets.

**Blocked by:** None — can start immediately.

**Status:** ready-for-agent

- [ ] Card shows Protected / Paused / Unprotected per the Axum API status payload, including remaining pause time from either the until-timestamp or duration fields
- [ ] Enable and disable actions update the card state after success and surface errors inline without leaving the tab
- [ ] Pause popup offers 5/15/30/60 minutes; after pausing, the card shows the pause state with countdown text
- [ ] A failed status load shows a retry affordance; retry recovers without app restart
- [ ] Pull-to-refresh refetches status; returning the app to the foreground also triggers a refetch
- [ ] Adapter methods exist for status/enable/disable/pause with response-shape guards rejecting malformed payloads as failures
- [ ] Application state machine tested against a scripted fake of the Axum API adapter interface
