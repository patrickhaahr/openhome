# Executor v2 dashboard agent guide

Applies to this directory and its descendants. Follow the repository's root `AGENTS.md` as well. Read this guide and [README.md](README.md) before editing the prototype. The current visual baseline is [the final screenshot gallery](screenshots/final/README.md).

## Scope and product constraints

This is an isolated Executor v2 experiment, not the production TanStack Start dashboard in `../../dashboard/`. It does not choose a replacement for TanStack Start. OpenHome's Axum API remains the production boundary for clients; do not bypass it with browser-to-device, LAN, bridge or database integrations. See the [dashboard release specs and tickets](https://github.com/patrickhaahr/openhome/issues/69), [API boundary ADR](../../docs/adr/0003-dashboard-api-boundary.md) and [household ownership ADR](../../docs/adr/0004-household-users-and-list-ownership.md).

Target: iPad 6th generation, iPadOS 17.7.11, primarily 1024×768 landscape CSS pixels. Also inspect 768×1024 portrait and narrow screens. Chromium screenshots are layout evidence; physical iPad/Safari rendering and performance still need verification.

Preserve the agreed design:

- Black/charcoal surfaces with restrained soft-yellow accents and semantic color tokens.
- Compact sidebar: Phosphor icon above a short one-word label, live time at its bottom. No sidebar brand, profile picker, breadcrumb, greeting or personal name.
- One shared household dashboard. Wishlist and Watchlist show multiple anonymous user sections together; ownership belongs to entries and writes, not a global active user.
- Home has separate time/date and seven-day weather sections, a clickable standalone agent orb, and four cards in two rows at both iPad sizes. Controls and routes are above; the larger exercise graph and bus arrivals are below. Narrow screens stack naturally.
- Calendar, Wishlist, Watchlist and Photos stay in their dedicated pages, with no Home cards. No Home navigation-arrow decorations. Watchlist's page heading is available to screen readers without duplicating the sidebar label visually.
- Lights use explicit ON/OFF, TV uses POWER, and speaker controls provide optical, Bluetooth, volume up/down. Workout shows independently toggleable exercise lines, including Pull-ups and Dips.
- Routes and transit share a full-content map destination. Keep required map attribution visible: beneath the Home map, in the map control on the full map page.
- Keep UI copy short and useful. Do not put build, API or architectural details into ordinary product flows.

The repository is public. Use synthetic data, anonymous labels, public Immich demo data when needed and unrelated public Copenhagen coordinates. Never add household locations, names, private photos, calendar entries, viewing history, credentials, pairing links or tokens to source, docs, screenshots or logs. Services must remain free under the current product constraint. [Subscription-backed Hermes live voice](https://github.com/patrickhaahr/openhome/issues/85) is required for production; paid API fallback is excluded. This prototype has no connected Hermes/live voice.

## How this prototype runs

Verified baseline: Executor `2.0.0-beta.14`, framework `apps` `0.0.1-beta.69`, Node 24. Exact dependency versions live in `app/package.json` and the local justfile. Do not silently update them or install `apps@latest`.

There are two different code histories: this OpenHome repository and Executor's private app source history. Editing this repository alone does not rebuild the running app. `tools/deploy.mjs` discovers/reuses the local app, reads its current source revision, commits the complete `app/` directory using `--expected`, then deploys that commit. Those Executor commits are not commits in this repository. A successful local deployment reloads an already-open app page automatically; it is not registry publishing or production deployment.

`serve` starts the local Executor host on loopback port 4312. Its runtime, app database, keys and local app ID live under ignored `.runtime/`. `tools/executor.mjs` reads the local key internally and invokes the pinned CLI. Never print or copy `.runtime/data/keys.json`. `tools/pair-browser.mjs` opens a one-use pairing URL in the named browser session without printing the token.

The host builds both the server entry `app/index.ts` and the React browser UI `app/ui/index.html` / `main.tsx`. There is no Vite/TanStack dev server in this prototype. Source edits become visible through `deploy`.

## Commands

Run from the OpenHome repository root, inside devenv, using the prototype justfile. Add a recipe if one is missing; do not bypass existing recipes with raw package commands.

```sh
# Initial setup; repeat only when needed.
devenv shell -- just --justfile prototypes/executor-dashboard/justfile install-runtime
devenv shell -- just --justfile prototypes/executor-dashboard/justfile install-app

# Keep this running in its own terminal.
devenv shell -- just --justfile prototypes/executor-dashboard/justfile serve

# Verify, then update the local app.
devenv shell -- just --justfile prototypes/executor-dashboard/justfile fmt
devenv shell -- just --justfile prototypes/executor-dashboard/justfile check
devenv shell -- just --justfile prototypes/executor-dashboard/justfile deploy

# Read the running host's framework version or discover CLI signatures.
devenv shell -- just --justfile prototypes/executor-dashboard/justfile executor apps framework
devenv shell -- just --justfile prototypes/executor-dashboard/justfile executor apps --help
```

For a human browser, `executor pair` creates a sensitive one-use URL: open it privately, select the prototype and open its app UI. Do not paste that URL into chat, docs or screenshots. For the automated fallback browser, use `pair-browser` instead. Local app IDs and URLs change on a fresh runtime; obtain the actual app URL from Executor rather than copying a previous machine's URL.

Keep the server alive while deploying and browsing. When backgrounding it, redirect logs to ignored `.runtime/serve.log` and retain the process/session handle. For access from another computer, forward port 4312 to host loopback; actual iPad hosting requires its own HTTPS/origin configuration. Do not expose the local development server to the network as a shortcut.

## Source map and extension points

| File | Responsibility |
| --- | --- |
| `app/index.ts` | `defineApp`, server query/mutation tools, isolated demo SQL access. |
| `app/schemas.ts` | Shared input/output schemas used by server and browser. |
| `app/migrations/` | Executor demo SQLite migrations; separate from production `api/migrations/`. |
| `app/ui/main.tsx` | React entry, typed Executor client, shared control/watchlist state, URL navigation, focus handling. |
| `app/ui/navigation.ts` | Page union, sidebar destinations and short labels. |
| `app/ui/feature-pages.tsx` | Page dispatch and dedicated weather/controls/workout/agent/integration views. |
| `app/ui/overview.tsx` | Two-row Home composition and clickable avatar. |
| `app/ui/controls.tsx`, `weather.tsx`, `clock.tsx` | Reusable controls, seven-day forecast and shared minute-aligned local clock. |
| `app/ui/travel.tsx`, `map.tsx`, `demo-route.ts` | Routes/transit modes, MapLibre lifecycle, 2D/3D views and public demo geometry. |
| `app/ui/panels.tsx` | Wishlist, route, transit, exercise chart and agent demo components. |
| `app/ui/household.ts`, `watchlist.tsx` | Anonymous member labels, entry ownership and in-memory watch-status flows. |
| `app/ui/style.css`, `map-controls.css` | Semantic colors, layout, focus/motion rules and minimal MapLibre controls. |
| `tools/` | Local CLI authentication, complete-source deployment and browser pairing wrappers. |

Reuse shared widgets rather than duplicating Home and feature-page behavior. For a new page, extend `navigation.ts`, handle the new union member in `FeaturePage`, keep an accessible `h1`, and capture its actual rendered UI. Preserve `?page=...`, browser back/forward, modified-click link behavior and heading focus. Navigation IDs are `home`, `travel`, `weather`, `controls`, `workout`, `wishlist`, `watchlist`, `photos`, `calendar`, `agent`.

The browser uses `createAppClient` from `apps/client`, typed query/mutation references, shared schemas and `useAppQuery` from `apps/react`. Its server imports are type-only; do not bundle server credentials or runtime code into the UI. Wishlist writes create an ID and call the demo mutation; the server binds SQL parameters. Failed saves show an uncertain-result message rather than claiming success.

Only the wishlist persists in Executor's demo SQL store. Watchlist and controls survive page navigation but reset on reload. Exercise toggles and travel modes belong to their page components. Weather, exercise values, route targets, transit estimates and agent voice states are illustrative. Changing a route target does not generate a new route. Photos and Calendar are status pages, not completed integrations. Do not turn sample bus estimates or synthetic fitness values into claims of live data.

The old synthetic demo owner IDs are still present in the applied migration and stored rows; `household.ts` displays them as User 1/User 2. They are not actual household identities. Do not edit an applied migration to rename IDs; add a migration and update the mapping if needed. Likewise, changing the prototype watchlist does not implement the planned production watchlist migration.

`addWish` uses `approval: never()` for this explicitly isolated synthetic demo store. This is not a default permission policy for future account or device writes. Follow existing user authorization and operation semantics when adding a real integration. Do not infer permission from this guide or a provider's tool metadata.

## Executor reference rules

Before adding framework capabilities or upgrading, inspect the installed framework and host CLI and consult the official [app authoring guide](https://executor.sh/docs/author-an-app) and [apps/deployments guide](https://executor.sh/docs/concepts/apps-and-deployments). The current docs may describe newer behavior than this pinned beta.

Official authoring essentials: pin the host's exact `apps` release; use `defineApp`, schemas and named query/mutation tools; declare provider account slots instead of embedding credentials. Keep migration files append-only. Source commits replace a complete file set, so preserve every required file and check removed paths. React UI ships with the app; SSR/RSC are unsupported. Saving source and deploying are separate operations. Re-discover tool signatures after changes. These are framework references, not evidence that this prototype already connects accounts.

## Runtime and UI gotchas

- Node 24.14+ is required. The repo's devenv supplies Node 24.
- On this Nix/Linux host, workerd dependency downloads require system CA certificates. `serve` sets `NODE_EXTRA_CA_CERTS` to `/etc/ssl/certs/ca-certificates.crt` unless already set. Do not disable TLS verification.
- Executor beta14 rejects CSS `data:` imports in MapLibre's default stylesheet. Keep the minimal `map-controls.css`; importing the full default CSS breaks this verified runtime.
- Use direct Phosphor CSR icon imports and the existing `IconContext`. No additional icon package or speculative UI framework is needed.
- The basemap requires network/WebGL. The browser recipe uses system Chromium plus SwiftShader; this verifies the map, not iPad GPU performance. Its upstream missing `wood-pattern` sprite warning is nonfatal.
- Map instances must be removed on unmount. The Home route fits its bounds again after resizing. Preserve map loading/error states and reduced-motion camera behavior.
- Keep minute-aligned clock updates and foreground refresh instead of continuous one-second rerenders. Home and sidebar share the same current time.
- Main content and transit overlays scroll independently. A browser's full-page screenshot does not necessarily reveal overflow inside those containers; inspect their scroll dimensions and scroll them deliberately if needed.
- The current fonts load from Google Fonts with system fallbacks; do not claim offline operation. Preserve readable fallback sizing and the semantic palette rather than adding arbitrary hex colors.
- Use at least 44×44 CSS-pixel interactive targets, explicit icon-button names, pressed states, visible focus and reduced-motion support. Hidden page headings are intentional.

## Verification and final screenshots

For UI-only changes, run `fmt` and `check`, deploy locally, then inspect the affected pages in a real browser. Test actual behavior that changed, not implementation-shaped assertions. If editing production areas too, read their `AGENTS.md` and run their own required gates.

Prefer the product-native collaborative browser when available. Check its status and try opening it before falling back. On a headless host where it explicitly reports unavailable, the existing agent-browser recipe is the verified fallback:

```sh
devenv shell -- just --justfile prototypes/executor-dashboard/justfile install-browser
devenv shell -- just --justfile prototypes/executor-dashboard/justfile browser skills get core
devenv shell -- just --justfile prototypes/executor-dashboard/justfile pair-browser
# Use the actual app URL obtained above; this placeholder is not a runnable URL.
devenv shell -- just --justfile prototypes/executor-dashboard/justfile browser open '<app-ui-url>?page=home'
devenv shell -- just --justfile prototypes/executor-dashboard/justfile browser set viewport 1024 768
devenv shell -- just --justfile prototypes/executor-dashboard/justfile browser snapshot -i
# Refresh snapshot refs after navigation. Wait for map readiness on mapped pages.
devenv shell -- just --justfile prototypes/executor-dashboard/justfile browser wait '[data-map-state="ready"]'
devenv shell -- just --justfile prototypes/executor-dashboard/justfile browser screenshot screenshots/final/home.png
devenv shell -- just --justfile prototypes/executor-dashboard/justfile browser errors
```

The justfile runs recipes in this directory, so `screenshots/final/home.png` resolves here. Its browser session is named `openhome-executor-ui`; do not use an unnamed global session or print authentication state. Pair again when authentication expires. Capture real browser PNGs, not generated mockups. Wait for the wishlist query, fonts and map to settle; reset only transient UI state, never delete stored wishes to make a screenshot prettier.

Before finishing: verify no horizontal overflow, Home's two rows and seven-day forecast at both iPad sizes, the affected navigation/actions, and the absence of browser page errors. Check PNGs visually and for private content. Replace only the final references affected by the change and update the gallery notes. `screenshots/final/` is tracked; other captures, dependencies and `.runtime/` stay ignored.

[Final PNG gallery](screenshots/final/README.md) includes all ten pages, both travel modes, a 3D route view and Home in portrait. Images show this prototype's final current UI, including unfinished integration status pages, and do not imply a feature release.
