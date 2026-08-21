Status: ready-for-agent

# Spec: Server operations tabs in `mobile-expo`

## Problem Statement

The Expo Mobile Client currently controls room devices only (IR remote, lights, geofence). The homelab's server-side surfaces — AdGuard Protection, the Docker Containers, and the RSS Compact Timeline — live in the deprecated Tauri client, which is unmaintained. To pause ad blocking, restart an unhealthy Container, or read the timeline, the owner must open a legacy app that will rot.

## Solution

Two new Top-Level Tabs appended to the existing pager: a **Server Tab** and a **Docker Tab**.

The Server Tab groups the mixed server-operations surfaces: an AdGuard Protection card (protected / paused / unprotected with remaining pause time, enable / disable / pause 5–15–30–60 minutes), a Docker Health Summary card (healthy / unhealthy / idle / offline plus running-vs-total count) that jumps to the Docker Tab on tap, and the Compact Timeline with an inline collapsible Feed manager (add by URL, delete with undo).

The Docker Tab lists Containers with run state, health, image, uptime, ports, and restart counts, filterable by health classification; selecting a Container shows its logs (fixed tail of 200 lines, timestamps on, manual refresh); each Container offers immediate start / stop / restart with no confirmation step.

All data comes from the existing Axum API endpoints; no backend work. Every surface supports pull-to-refresh and refetches when the app foregrounds or the tab regains focus. The deprecated Tauri client is left untouched.

## User Stories

1. As a homelab owner, I want to see whether AdGuard Protection is currently enabled, paused, or disabled, so that I know whether my network is filtered at a glance.
2. As a homelab owner, I want to see how much time remains on a Protection Pause, so that I know when filtering resumes.
3. As a homelab owner, I want to disable AdGuard Protection with one tap, so that I can troubleshoot network issues quickly.
4. As a homelab owner, I want to re-enable AdGuard Protection with one tap, so that I restore filtering when done troubleshooting.
5. As a homelab owner, I want to pause AdGuard Protection for 5, 15, 30, or 60 minutes, so that it resumes automatically instead of staying off.
6. As a homelab owner, I want to see the AdGuard version and whether it is running, so that I can tell "paused" apart from "AdGuard itself is down".
7. As a homelab owner, I want a retry affordance when AdGuard status cannot be fetched, so that a transient failure doesn't force me to leave the tab.
8. As a homelab owner, I want a single Docker Health Summary on the Server Tab, so that I can check overall container health without opening the Docker Tab.
9. As a homelab owner, I want the summary to distinguish healthy, unhealthy, idle (nothing running), and offline (Axum API unreachable), so that the headline matches reality.
10. As a homelab owner, I want the running-vs-total Container count under the summary, so that I can spot stopped services even when health looks fine.
11. As a homelab owner, I want to tap the summary to jump to the Docker Tab, so that drill-in is one gesture.
12. As a homelab owner, I want to see every Container with name, image, run state, health status, uptime, ports, and restart count, so that I can assess my stack from my phone.
13. As a homelab owner, I want to filter the Container list by health classification, so that I can find problem containers in a long list.
14. As a homelab owner, I want per-classification counts alongside the filters, so that I know what each filter will show before tapping.
15. As a homelab owner, I want to read a Container's recent logs (last ~200 lines, timestamped), so that I can diagnose crashes without SSH.
16. As a homelab owner, I want to refresh logs manually, so that I can re-check after reproducing a problem.
17. As a homelab owner, I want start / stop / restart to execute immediately with no confirmation dialog, so that fixing my own homelab takes one tap.
18. As a homelab owner, I want distinct errors for "container not found" versus "Docker unavailable" versus other failures, so that I know what actually went wrong.
19. As a homelab owner, I want to scroll the newest Feed items in a Compact Timeline, so that I can skim what happened on my feeds.
20. As a homelab owner, I want older timeline items to load as I reach the end of the list, so that reading history doesn't require a separate page.
21. As a homelab owner, I want to tap a timeline item to open its link in the system browser, so that I read full articles where they live.
22. As a homelab owner, I want to add a Feed by pasting its URL, so that new sources join the Compact Timeline.
23. As a homelab owner, I want to delete a Feed, so that sources I no longer follow stop appearing.
24. As a homelab owner, I want to undo a Feed deletion, so that an accidental delete costs nothing.
25. As a homelab owner, I want feed management inline on the Server Tab rather than behind a modal, so that adding and pruning feeds stays lightweight.
26. As a homelab owner, I want invalid feed URLs rejected with a clear message, so that I don't add broken subscriptions.
27. As a homelab owner, I want pull-to-refresh on every server-operations surface, so that I control when data updates.
28. As a homelab owner, I want data refetched automatically when the app returns to the foreground, so that what I see isn't stale after the phone sat idle.
29. As a homelab owner, I want loading and error states rendered inline per surface, so that one failing area never blanks out the whole tab.

## Implementation Decisions

- **Navigation**: Two Top-Level Tabs appended after Away (`Home · TV · Speaker · Away · Server · Docker`) inside the existing custom pager. No router dependency added.
- **Server Tab** is a deliberate mixed Top-Level Tab — the same role for server ops that the Home Tab plays for quick access. This resolution is recorded in `CONTEXT.md` along with the new terms (Server Tab, Docker Tab, Container, Docker Health Summary, AdGuard Protection, Protection Pause, Feed, Compact Timeline).
- **API adapter**: extend the single shared Axum API adapter (the configuration-bound fetch client that already handles Bearer auth, timeouts, JSON guards, and `Result` returns) with namespaced method groups: `adguard` (status, enable, disable, pause), `docker` (list, logs, start, stop, restart), `rss` (compact timeline, list/create/delete feeds). Response-shape guards reject malformed payloads as failures; docker container parsing tolerates both PascalCase and snake_case field spellings inherited from the old wire contract. Request timeout is raised to ~15s for docker calls; 5s elsewhere.
- **Application layer**: one reducer state machine hook per feature (AdGuard, Docker, RSS), following the existing typed-event-union pattern. Machines own orchestration: initial load, pull-to-refresh, focus/foreground refetch (wired through the existing AppState resume point), timeline pagination via `before_id`, feed delete + undo (undo re-adds deleted Feeds), and immediate lifecycle dispatch.
- **No confirmation on lifecycle actions**: stop/restart/start dispatch instantly. Deliberate choice for a single-user homelab.
- **Logs**: fixed tail (200), timestamps always on, one-shot fetch + manual refresh, no auto-scroll toggle, no `since` filtering. Rendered by swapping the Container list view inside the Docker Tab; back returns to the list.
- **Links**: timeline items open via React Native `Linking.openURL` (system browser).
- **Feed manager UI**: inline collapsible section on the Server Tab — no Modal component introduced.
- **Theme**: status semantics map onto the existing palette tokens (ready/signal/danger); no theme additions.
- **Endpoints consumed** (all pre-existing): `GET /api/adguard/status`, `POST /api/adguard/enable`, `POST /api/adguard/disable`, `POST /api/adguard/pause`; `GET /api/docker`, `GET /api/docker/:name/logs?tail&timestamps`, `POST /api/docker/:name/start|stop|restart`; `GET /api/timeline?view=compact&limit&before_id`, `GET/POST /api/feeds`, `DELETE /api/feeds/:id`.
- No ADRs were warranted; all decisions here are reversible.

## Testing Decisions

- Good tests assert external behavior only: given scripted adapter responses (or scripted events), what state the machine exposes and what commands it issues — never internal call ordering or reducer shape.
- **S1 (primary seam)**: each application state machine tested against a scripted fake of the Axum API adapter interface. Covers loading/error surfacing, refresh triggers, pause flows, pagination, delete-with-undo, and lifecycle dispatch.
- **S2 (secondary seam)**: adapter response parsing tested through stubbed global fetch — only for the tricky wire-shape guards (docker container normalization incl. case fallbacks, compact-timeline array validation, AdGuard status shape, error-body extraction).
- Prior art: colocated vitest tests next to the existing application state-machine hooks and infrastructure adapters; run via the repo's standard expo test command (`just expo-check` covers typecheck + tests + export).

## Out of Scope

- Auth/biometric/unlock flow, settings/configuration screens, random fact card, API status dot and health polling — deliberately omitted from this migration (the Expo client already has setup/validation).
- Any backend/API change; all endpoints already exist.
- Changes to the deprecated Tauri client — it stays frozen.
- Log streaming/live tailing, tail-size picker, `since` filtering.
- Periodic background polling timers.
- Full-article reading, feed-item read state, bookmarks.
- Confirmation dialogs for any destructive action.

## Further Notes

- The old client's desktop-only refresh buttons are superseded by pull-to-refresh and are not ported.
- `mobile-native` does not receive these tabs in this effort; capability parity there is a separate future decision.
- Wire-contract detail worth preserving in review: the docker list payload historically mixes PascalCase (`HealthStatus`, `Created`) and snake_case fields; the parser's tolerance is intentional.
