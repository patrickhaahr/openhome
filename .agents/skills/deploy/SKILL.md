---
name: deploy
description: Deploy OpenHome when asked to deploy or release this repository to the Raspberry Pi, including automatic patch bumps, local multi-platform builds and live verification.
---

# Deploy OpenHome

Treat a request to "deploy" in this repository as authorization to release the
current working tree: patch-bump API, MCP and dashboard, publish their images,
back up both databases and restart the stack on `pi`.

1. Inspect `git status --short` and `git diff` to establish what will ship. Run
   `devenv shell -- just deploy` from the repository root.
2. Let the command finish. It owns version and lockfile updates, checks, local
   amd64/arm64 release builds, publishing, dashboard image smoke testing, SSH,
   SQLite backups, Compose activation and live verification. Success requires
   exit code zero and the reported versions running with healthy containers,
   matching image IDs and zero restarts; authenticated API/MCP reads and HTTPS
   dashboard assets must also pass.
3. Report the deployed versions, verification result and backup location.
   State any remaining limitation explicitly.

If a run fails, diagnose the failed phase before retrying. Use
`devenv shell -- just deploy-current` to reuse the already bumped versions.
Use `devenv shell -- just openhome-verify` to recheck the running release without
restarting it. Keep backup paths and previous image IDs available when diagnosing
an activation failure; database restoration requires a deliberate recovery plan.

The justfile and scripts are the source of truth for individual commands.
Builds use the single-node `openhome-local` builder; the Pi runs production
containers. Physical iPad/Safari acceptance remains a separate release check.
