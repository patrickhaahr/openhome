# Executor prototype guidance

- Read `README.md` before editing; for UI changes, inspect `screenshots/final/README.md` for the visual baseline. This is an isolated demo, separate from the production dashboard.
- Use `just --justfile prototypes/executor-dashboard/justfile <recipe>` from the repo root. Run `fmt` and `check` after changes.
- Source edits require `deploy` to update the running app; keep `serve` running. Deployment replaces the complete app source set in Executor's separate history.
- Keep runtime keys and pairing URLs private. Use `pair-browser` for automation; keep the host on loopback and use synthetic, anonymous data in code and captures.
- Before adding Executor capabilities or upgrading, inspect the installed framework and CLI and consult the authoring reference linked in `README.md`; preserve pinned versions unless upgrading is requested.
- Keep demo SQL migrations append-only. Real device and service integrations belong behind the Axum API; demo mutations do not establish permission for real writes.
- After UI changes, deploy locally and inspect affected behavior at 1024×768, 768×1024 and a narrow viewport. Check overflow, browser errors and private content; update affected final PNGs and gallery notes. Chromium checks leave physical iPad/Safari verification pending.
