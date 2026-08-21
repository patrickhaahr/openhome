# AGENTS.md

Start here for repo-wide guidance. Then read `api/AGENTS.md` or `mobile-expo/AGENTS.md` before changing those areas.

## Current Repo Reality

- `mobile-expo/` is the active mobile client.
- `mobile/` is the old Tauri/SolidJS client and is deprecated. Do not treat it as the default mobile target unless the task explicitly says so.
- The root `README.md` is stale and still describes the deprecated Tauri client. Prefer `justfile`, `flake.nix`, `CONTEXT.md`, and the service-specific `AGENTS.md` files as sources of truth.

## Commands

- Prefer `just` recipes from the repo root over ad-hoc CLI commands.
- API:
  - `just run`
  - `just test`
  - `just test-one <name>`
  - `just test-integration <name>`
  - `just fmt`
  - `just lint`
  - `just go`
- Expo (`just groups` lists everything grouped by area):
  - `just expo-install`
  - `just expo-start`
  - `just expo-android`
  - `just expo-typecheck`
  - `just expo-lint`
  - `just expo-test [<filter>]`
  - `just expo-check`

## Verified Workflow Gotchas

- `flake.nix` is the real dev-shell setup: it installs `just`, Rust tooling, JDK 17, `adb`, and the `android` CLI shim (used by `expo-android` to locate the SDK).
- The dev shell exports `DATABASE_URL="sqlite:$repo_root/api/data/app.db"`. API commands and tests may rely on that instead of a manually exported path.

## Architecture Notes

- `api/` is a standalone Rust Axum service. `api/migrations/` is live SQLx migration state.
- `mobile-expo/` is a standalone Expo app. Keep domain rules, application state, infrastructure adapters, and UI components separated under `src/`.
- Use the domain language in `CONTEXT.md` when changing product behavior.
- Mobile clients talk only to the Axum API. Do not add direct device, bridge, or LAN integration code to `mobile-expo/`.

## Verification Bias

- For API work, default to the smallest relevant `just test...` command, then run `just lint` if Rust code changed.
- For Expo work, default to `just expo-lint` and `just expo-test <filter>`; use `just expo-check` as the full gate (typecheck + tests + Android export) when touching shared infrastructure or shipping.
