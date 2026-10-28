---
name: deploy
description: Deploy OpenHome when asked to deploy or release this repository to the Raspberry Pi, including patch bumps of only the changed images, local multi-platform builds and live verification.
---

# Deploy OpenHome

Treat a request to "deploy" in this repository as authorization to release the
current working tree: patch-bump, publish and restart only the images whose area
changed, and back up both databases on `pi`.

An area (`api/`, `mcp/`, `dashboard/`) is released when it has committed,
uncommitted or untracked changes since the commit that set its current version.
MCP is always released with API because its image compiles the API crate. A
dashboard-only change bumps and restarts only the dashboard; API and MCP keep
their versions and running containers.

1. Inspect `git status --short` and `git diff` to establish what will ship. Run
   `devenv shell -- just deploy` from the repository root. It prints the areas
   and versions it releases; stop it if that differs from what changed. To force
   an area, name it: `devenv shell -- just deploy api`.
2. Let the command finish. It owns version and lockfile updates, checks, local
   amd64/arm64 release builds, publishing, dashboard image smoke testing, SSH,
   SQLite backups, Compose activation and live verification of the whole stack.
   Success requires exit code zero and all three reported versions running with
   healthy containers and matching image IDs, with zero restarts for the
   released ones; authenticated API/MCP reads and HTTPS
   dashboard assets must also pass.
3. Commit the version bump (manifests and lockfiles of the released areas) as
   `chore(release): ...`; that commit marks the release for the next deploy.
   Report the deployed versions, verification result and backup location.
   State any remaining limitation explicitly.

If a run fails, diagnose the failed phase before retrying. Use
`devenv shell -- just deploy-current` to reuse the already bumped versions.
Use `devenv shell -- just openhome-verify` to recheck the running release without
restarting it. Keep backup paths and previous image IDs available when diagnosing
an activation failure; database restoration requires a deliberate recovery plan.

The justfile and scripts are the source of truth for individual commands.
Builds use the single-node `openhome-local` builder; the Pi runs production
containers. Physical iPad/Safari acceptance remains a separate release check.
