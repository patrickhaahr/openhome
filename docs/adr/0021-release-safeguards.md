# Release safeguards

`scripts/deploy.py` and `scripts/deploy-pi.py` release to a single Pi with no
staging environment, so every step either leaves production running or
restores it.

- **Before downtime.** Every release tag is resolved before the API stops, so
  a missing tag leaves production untouched. Every manifest and lockfile is
  validated before any of them is written. A failed release keeps its bumped
  versions, and `deploy-current` resumes them. Builder setup is also called by
  recipes inside the locked release, so it does not take the release lock.
- **Backups.** Data copies, including SQLite's backup, happen inside the
  stopped window. The backup owns its copy of meal photos before the API
  resumes, because a resumed API may replace or delete originals. Meal photo
  references must be UUIDs, so a malformed database value cannot point a backup
  path outside the photo directory. The first release with nutrition backs up a
  schema without Meal Logs or photos. The previous API is started again even
  when a missing photo or a failed disk aborts the release.
- **Compose.** Release-owned configuration is installed only after the previous
  stack is preserved. `COMPOSE_FILE` in the private `.env` opts into the
  Bluetooth overlay. The container's numeric UID must exist on the host,
  because system D-Bus authenticates it before BlueZ policy applies. Only
  released services are pulled, and `up` recreates only containers whose image
  or configuration changed. A container left running keeps its restart
  history; only a freshly created one must report zero restarts.
- **Coupled images.** The MCP image compiles the API crate and locks its
  version, so releasing the API releases MCP.
- **Signing.** Mobile release signing refuses an existing or partial identity,
  because silently rotating a key breaks upgrades. The release script never
  enables shell tracing, which would print secrets.
- **Tests.** Deploy tests run fixture commits with global git config turned
  off, so the developer's own hooks do not judge them.
