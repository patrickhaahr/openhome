# OpenHome verification map

The maintained source for verifying OpenHome's user-facing behaviour on the synthetic stack. Read this index, then use the matching feature file as the recipe.

## Baseline preconditions

- A run started by this checkout's `scripts/up.sh`, with `scripts/doctor.sh` printing `OK`.
- Seed Household Users are Alex, Robin and Sam. Look up their IDs with `mcp.sh call household_users '{}'`; IDs change on every launch.
- Data is in memory and resets on relaunch, so restart rather than undo mutations.

## Driving conventions

- Hermes paths go through `scripts/mcp.sh`. Read-backs use curl against `$API_URL` with `$API_KEY`. Dashboard paths use the T3 Code preview tools on `$DASHBOARD_URL`'s port.
- Prefer accessible names from `preview_snapshot` over coordinates.
- Use titles and request IDs that start with `Verify` / `verify-<run id>` so proof is unambiguous against seed data.

## Proof and skip reporting

- Capture each tool response, an independent read-back, and a Dashboard snapshot or screenshot when the feature has a Dashboard view.
- Record the feature and entry point with each artifact under `$EVIDENCE_DIR/<feature>/`.
- Report an entry point you couldn't drive, with the unmet precondition. Don't report it as verified through another path.

## Feature entry contract

Each feature file has an H1, one paragraph, then exactly four H2s in order: `Sub-features`, `How to get to it (user POV)`, `Driving it with verify-openhome`, `Gotchas`.

## Features

- [Watchlist](./watchlist.md): adding through Hermes with retry replay, Dashboard Watch view. **Proven 2026-11-13.**
- [Wishlist](./wishlist.md): wishes through Hermes and the Dashboard Wish view.
- [Important Dates](./important-dates.md): dates through Hermes and the Dashboard Dates view.
- [Meal logging](./meal-logging.md): Food search and meal logs through Hermes, the Dashboard Food page.

## Planned: Binder (epic #285)

Binder tickets must add `binder.md` here once a slice is driveable. They must also add loopback stand-ins for the OPTCG catalogue and the ECB rates file to the synthetic preview example, the same way TMDB has one, so card search, images, prices and currency values work offline with invented cards. Never point the preview at the live OPTCG service: it is one volunteer's server.
