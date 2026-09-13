# AGENTS.md

Start here for repo-wide guidance. Then read `api/AGENTS.md` or `mobile-expo/AGENTS.md` before changing those areas.

## Current Repo Reality

- `mobile-expo/` is the active mobile client.
- `mobile/` is the old Tauri/SolidJS client and is deprecated. Do not treat it as the default mobile target unless the task explicitly says so.
- The root `README.md` is stale and still describes the deprecated Tauri client. Prefer `justfile`, `devenv.nix`, `CONTEXT.md`, and the service-specific `AGENTS.md` files as sources of truth.

## Commands

- All commands run inside the devenv environment. Enter it first with `devenv shell`, or prefix one-off commands with `devenv shell --` (e.g. `devenv shell -- just test`).
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

- `devenv.nix` is the dev-shell setup: Rust comes from `languages.rust` (stable), Expo tooling from `bun`/`watchman`, and the API's native deps (`openssl`, `pkg-config`, `sqlx-cli`, `sqlite`) from `packages`.
- The Android SDK is provided by the devenv `android` integration (not `~/Android/Sdk`): it exports `ANDROID_HOME`/`ANDROID_SDK_ROOT`/`JAVA_HOME` and puts `adb`/`sdkmanager` on PATH. Versions are pinned to match React Native 0.86 (platform 36, build-tools 36.0.0, NDK 27.1).
- The dev shell exports `DATABASE_URL="sqlite:$repo_root/api/data/app.db"`. API commands and tests may rely on that instead of a manually exported path.
- `devenv test` runs the full gate: git hooks (`just fmt` + `just lint`) and `enterTest` (`just test`).
- Java (JDK 17) is only needed for native Android builds (`expo run:android` / `just expo-android`, via Gradle). Typecheck, lint, and tests don't need it.

## Architecture Notes

- `api/` is a standalone Rust Axum service. `api/migrations/` is live SQLx migration state.
- `mobile-expo/` is a standalone Expo app. Keep domain rules, application state, infrastructure adapters, and UI components separated under `src/`.
- Use the domain language in `CONTEXT.md` when changing product behavior.
- Mobile clients talk only to the Axum API. Do not add direct device, bridge, or LAN integration code to `mobile-expo/`.

## Verification Bias

- For API work, default to the smallest relevant `just test...` command, then run `just lint` if Rust code changed.
- For Expo work, default to `just expo-lint` and `just expo-test <filter>`; use `just expo-check` as the full gate (typecheck + tests + Android export) when touching shared infrastructure or shipping.
