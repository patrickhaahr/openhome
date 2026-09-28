# AGENTS.md

Start here for repo-wide guidance. Then read `api/AGENTS.md`, `mcp/AGENTS.md` or `mobile-expo/AGENTS.md` before changing those areas.

## Current Repo Reality

- `mobile-expo/` is the active mobile client.
- `mobile/` is the old Tauri/SolidJS client and is deprecated. Do not treat it as the default mobile target.
- The root `README.md` is stale and still describes the deprecated Tauri client. Prefer `justfile`, `devenv.nix`, `CONTEXT.md`, and the service-specific `AGENTS.md` files as sources of truth.

## Commands

- All commands run inside devenv: `devenv shell -- <cmd>` for one-offs. `just groups` lists every recipe grouped by area.
- Run everything through the justfile — never raw `cargo test`, `cargo clippy`, `bun run lint`, etc. when a recipe exists. If a needed recipe is missing, add one to the justfile rather than bypassing it.
- Test: `just api-test` (or `api-test-one <name>`, `api-test-integration <name>`), `just mcp-test`, `just expo-test [<filter>]`, `just cli-test`.
- Verify after implementation: run the `-check` gate for each modified area — e.g. after touching `api/` and `mcp/`: `just api-check && just mcp-check`. Gates: `api-check`, `mcp-check`, `cli-check`, `mobile-check`, `firmware-check`; `just check` runs all.
- Runs/dev servers: `just api-run` (env comes from devenv), `just mcp-run` (needs `OPENHOME_API_URL` and `API_KEY`), `just expo-start`, `just expo-install`, `just expo-android`.

## Verified Workflow Gotchas

- `devenv.nix` is the dev-shell setup: Rust comes from `languages.rust` (stable), Expo tooling from `bun`/`watchman`, and the API's native deps (`openssl`, `pkg-config`, `sqlx-cli`, `sqlite`) from `packages`.
- The Android SDK is provided by the devenv `android` integration (not `~/Android/Sdk`): it exports `ANDROID_HOME`/`ANDROID_SDK_ROOT`/`JAVA_HOME` and puts `adb`/`sdkmanager` on PATH. Versions are pinned to match React Native 0.86 (platform 36, build-tools 36.0.0, NDK 27.1).
- The dev shell exports `DATABASE_URL="sqlite:$repo_root/api/data/app.db"`. API commands and tests may rely on that instead of a manually exported path.
- `devenv test` runs the full gate: the pre-commit git hook (`just check`) and `enterTest` (`just api-test`).
- Java (JDK 17) is only needed for native Android builds (`expo run:android` / `just expo-android`, via Gradle).
- Android emulator: `emulator -avd medium_phone -no-window -no-audio -no-snapshot` launches the existing headless AVD. devenv.nix's `enterShell` handles the needed env (`LD_LIBRARY_PATH` for the emulator's bundled libs and `ANDROID_AVD_HOME` pointing at `~/.android/avd`). Boot takes ~1-2 min; then `adb devices` shows `emulator-5554` and `just expo-android` can install/run on it.

## Architecture Notes

- `api/` is a standalone Rust Axum service. `api/migrations/` is live SQLx migration state.
- `mcp/` is the Hermes MCP adapter: a standalone Rust crate that turns read-only API training endpoints into MCP tools. It must not read the databases or interpret data; see `docs/training-context.md` and ADR 0002.
- `mobile-expo/` is a standalone Expo app. Keep domain rules, application state, infrastructure adapters, and UI components separated under `src/`.
- Use the domain language in `CONTEXT.md` when changing product behavior.
- Mobile clients talk only to the Axum API. Do not add direct device, bridge, or LAN integration code to `mobile-expo/`.
