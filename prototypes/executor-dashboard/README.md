# Executor v2 dashboard prototype

Start with [AGENTS.md](AGENTS.md) for the agent workflow and [the final PNG gallery](screenshots/final/README.md) for every page.

An isolated experiment built on Executor 2.0.0-beta.14 and its exact `apps` framework 0.0.1-beta.69. The TanStack Start dashboard template and OpenHome API remain the production starting point; this experiment does not decide their replacement.

The prototype uses a compact left sidebar with Phosphor icons above one-word labels. Home separates a live device-local clock and full date from today's weather and six upcoming days. Home has two rows of cards: controls and routes above, a wider exercise graph and estimated bus arrivals below. A standalone clickable agent orb sits beside the weather. Calendar, wishlist, watchlist and photos remain available through the sidebar. Map attribution appears beneath the Home map, without an overlay. Card headings have no navigation arrows.

The interface uses black surfaces and yellow accents. Home opens directly on the widgets, without a greeting or personal names. The dashboard has one shared household view. Wishlist and Watchlist show anonymous user sections together; additions and edits target the section's owner. Short labels and visible state replace decorative copy.

The refined palette uses charcoal surface layers, soft white text and a restrained yellow accent for selected navigation, active filters and enabled primary actions. Inactive icons, user badges and disabled actions stay neutral. Colors are defined through semantic tokens. Watchlist opens directly on user sections, with its page heading available to screen readers.

Rendered sidebar, active-filter, card-title, date, placeholder and disabled-action contrast pairs were measured in Chromium at 5.36–15.44:1. Home and the revised Watchlist fit the 1024×768 viewport without scrolling. These checks do not replace the pending physical iPad/Safari verification.

Dedicated pages cover routes/transit, weather, home controls, workout, wishlist, watchlist, photos, calendar and agent. Routes/transit fill the content area with a working public OpenFreeMap/MapLibre basemap. The agent has its own conversation and animated orb view. Photos and calendar show their integration status.

Sidebar links update `?page=travel` (or another page ID), support browser back/forward and survive reload. Changing pages focuses the page heading. Reduced motion disables continuous orb motion. Wishlist and Watchlist show all users by default, with independent watch-status filters inside each user's section. Lights have explicit ON/OFF actions, TV has POWER, and the speaker has optical/Bluetooth selection and volume up/down with a level indicator. These demo states and watchlist changes survive page navigation but reset on reload. Both clock placements update on minute boundaries and refresh when the tab returns to the foreground.

## Run

All commands start at the repository root and run in devenv:

```sh
devenv shell -- just --justfile prototypes/executor-dashboard/justfile install-runtime
devenv shell -- just --justfile prototypes/executor-dashboard/justfile install-app
devenv shell -- just --justfile prototypes/executor-dashboard/justfile check
devenv shell -- just --justfile prototypes/executor-dashboard/justfile serve
```

Keep `serve` running. In another terminal:

```sh
devenv shell -- just --justfile prototypes/executor-dashboard/justfile deploy
devenv shell -- just --justfile prototypes/executor-dashboard/justfile executor pair
```

Open the one-use pairing link printed by `pair`, select **OpenHome dashboard prototype**, then **Open app**. The app runs on its own loopback subdomain. `deploy` creates or updates the local app without publishing it; an active app page automatically reloads after a successful deployment. Local IDs and keys stay in ignored `.runtime/`.

The server binds loopback. For a browser on another computer, forward local port 4312 to this host's loopback port; pair in that browser. This is a local development prototype, not an iPad deployment. Private HTTPS/browser-origin configuration and a physical iPad Safari check belong to the deployment ticket.

## Runtime findings

- Node 24.14+ is required. This devenv currently supplies Node 24.
- On this Linux/Nix host, workerd needed `NODE_EXTRA_CA_CERTS=/etc/ssl/certs/ca-certificates.crt` to download npm dependencies. `serve` supplies that default while respecting an existing value. Without it, Executor's own bundled app fails during startup. TLS verification remains enabled.
- Executor beta14 rejects CSS `data:` URL imports, including MapLibre's default stylesheet. `ui/map-controls.css` supplies the small map/attribution layout needed by this prototype. No Executor runtime patch is required.
- Executor apps are React browser apps. TanStack Start server rendering is not part of this app authoring model. Production integration would need a separate decision and an Axum adapter.

References: [Executor app authoring](https://executor.sh/docs/author-an-app), [OpenFreeMap](https://openfreemap.org/), [MapLibre building extrusions](https://maplibre.org/maplibre-gl-js/docs/examples/display-buildings-in-3d/), [Phosphor React](https://github.com/phosphor-icons/react), [Immich public demo](https://demo.immich.app/).

## Scope and verification

The map uses unrelated public Copenhagen coordinates. Its loop is illustrative and does not regenerate when a target changes. Weather, transit, controls, agenda entries, exercise measurements and voice states are demo data. Only the wishlist is persisted, in Executor's isolated demo SQL store, with anonymous user sections. The watchlist demonstrates planned/watching/completed states and dates using fictional titles in memory; it does not implement the production database migration or TMDB/Jellyfin integration. Photos remain an unconnected dedicated page. The prompt shortcut calls a demo mutation; it is not a live Hermes conversation. No private accounts, microphones, device commands or paid services are connected. [Subscription-backed Hermes live voice](https://github.com/patrickhaahr/openhome/issues/85) is required for production; paid API fallback is excluded.

Home and the map page fit a 1024×768 Chromium viewport without content scrolling. Other feature pages use the main content area's scrolling when needed. The transit overlay scrolls independently over its map. Physical iPad/Safari performance is not yet verified. The upstream basemap emits a nonfatal missing `wood-pattern` sprite warning.

Verified in Chromium before the shared-view revision: sidebar destinations and active-page indication, heading focus on navigation, page selection after reload, public map load and route/transit 2D/3D controls, journey layout preview, demo control state across Home and its dedicated page, wishlist prompt mutation and persistence after reload, exercise toggles, simulated orb states and automatic reload after deployment. Formatting and TypeScript checks pass; no browser page errors were observed.

The shared-view revision was checked for both users appearing together, absence of a global profile selector, owner-specific wishlist and watchlist additions, independent watch-status filtering and status edits without changing another user's entries. Demo wishlist writes persist after reload; watchlist edits remain in memory.

The compact Home revision was checked at 1024×768, 768×1024, 320×568 and 512×384. Home fits the landscape iPad viewport; smaller views scroll without horizontal overflow. Icon labels remain visible, the forecast contains seven days, and device button targets are at least 44×44 pixels. RTL mirroring and German sidebar labels were checked for overflow. The reduced 512×384 layout checks the available space corresponding to 200% zoom, but actual browser zoom and physical iPad/Safari remain unverified. Explicit lights ON/OFF, TV power, speaker input selection, volume changes and 0–100 bounds were exercised. Device state survives navigation between Home and Controls; the exercise line toggles work in the preview and full graph. The public map loads without browser page errors.

The two-row Home revision fits both 1024×768 landscape and 768×1024 portrait without content scrolling. At 320×568 it stacks and scrolls without horizontal overflow. Browser checks confirmed the four Home cards occupy two rows at both iPad sizes, removed cards remain absent from Home, seven forecast days remain visible, the avatar opens Agent with heading focus, and Dips toggles its graph line. Pull-ups, Dips, Squat and Handstand use synthetic normalized measurements. Bus estimates remain sample data. The map credits sit outside the map canvas. Formatting and TypeScript checks pass; no browser page errors were observed.

Browser automation is optional: `install-browser`, then `pair-browser` or `browser <args>`. The recipe uses a system Chromium and an isolated named session. Final reference PNGs in `screenshots/final/` are included in version control. Temporary screenshots, browser artifacts, runtime keys and dependencies remain ignored. Screenshots are suitable for public review only while demo data remains synthetic.
