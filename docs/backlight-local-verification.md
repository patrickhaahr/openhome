# Backlight local verification — #209

Verified on the development host on 24 October 2026. This is the local readiness
handoff for #211, included in integration PR #212. No image was published and no
Raspberry Pi deployment or production schedule activation occurred.

The actual API and production-built Dashboard used source revision
`76e2a4b276fb7dc2cb0d30ae6eeaadce952aefe2`. This ticket changes only Docker build
staging, a local image-loading recipe and this evidence document; the verified API,
Dashboard and Bluetooth behavior is unchanged. The ticket completion comment and
integration merge identify the final build/documentation revision.

## Actual Dashboard, API and Bluetooth evidence

The production Dashboard ran locally against the actual authenticated Axum main
binary, an isolated migrated SQLite database and the development host's Bluetooth
adapter. Browser actions used the existing server-side private transport. These
were product interactions, not the earlier synthetic component preview.

- Off produced accepted writes and power-off readback. Red, green and blue UI
  selections produced accepted matching requested RGB commands and readable color
  mode; the brightness slider produced the requested 32% readback. The protocol
  does not establish exact visible RGB from its basic mode response.
- TV Sync restored powered-on video mode and the original saved brightness.
  Actual capture accepted TV Sync and rejected a static-mode replacement. The
  private durable profile remained byte-for-byte unchanged, including after live
  capture.
- The UI saved a temporary custom window and independent 27% bedtime brightness.
  Enabling applied Automatic; manual Off displayed its next-transition expiry;
  Automatic immediately cleared it and restored the current routine target.
- With Off selected, the browser was closed and the real API process was stopped
  and restarted against the same SQLite file. The unexpired Off override and exact
  expiry survived, with power-off readback. Without reopening the Dashboard, the
  actual wall-clock Start cleared Off and selected powered-on red at 27%; End
  restored TV Sync with its saved brightness. No database clock adjustment was
  used for these transitions.
- A second short window selected red at 37%. BlueZ blocked and disconnected only
  the target Backlight; other Bluetooth devices were not disabled. The actual
  Dashboard reported unreachable, stale observations and pending/retrying work.
  The outage spanned End: Axum saved the current TV Sync target while disconnected.
  Unblocking recovered TV Sync with fresh power/mode/brightness readback. This
  establishes recovery to the current target; deterministic transport tests also
  prove obsolete batches are never written.
- Actual authenticated API requests rejected equal times, malformed times,
  fractional bedtime brightness and out-of-range RGB without changing the saved
  policy. An unauthenticated request was rejected. The UI independently displayed
  its equal-time validation.
- A separate actual API process with a fresh isolated database and no device
  configuration reported incomplete setup, unconfigured reachability and unknown
  observations. Its real Dashboard explained setup, retained Off, and showed a
  failed Off command without inventing acknowledgment. Non-Off was rejected.
  Restoring the configured runtime recovered TV Sync.

Independent read-only Bluetooth snapshots before and after the routine/recovery
checks matched power, brightness, complete video-mode response, white balance,
whole-screen brightness and black-screen behavior exactly. Edge-brightness readout
was unsupported and is not claimed. These snapshots cover supported readable
settings, not camera geometric-calibration coordinates.

All 44 final private technical checks passed. Early private harness attempts
included transient pending responses and browser locator/scroll failures; the
final checks wait for authoritative recovery rather than treating a requested
selection as hardware confirmation. Browser screenshot/log artifacts and raw
settings remain private. The user explicitly waived human LED-color and
TV-picture/camera confirmation; no human visual observation is claimed.

Final restoration: Bedtime Mode disabled, 23:00–00:00 Europe/Copenhagen, red 20%,
Automatic, no manual override or next-transition cursor, powered-on saved TV Sync.
The target was unblocked. Local API, Dashboard and browser processes were stopped;
no temporary schedule remains running. Release any remaining development-host
Bluetooth connection before the Pi takes ownership in #211.

## Automated checks and local builds

- `devenv shell -- just api-check`: passed, including all 21 Backlight API tests
  and the existing wall-switch/IR suites. The existing authenticated Axum/SQLite,
  controlled time/device and focused calendar seams cover persistence, concurrent
  supersession, expired overrides, DST, stale/pending observations and retry order.
- `devenv shell -- just dashboard-check`: passed, 27 files / 193 tests and the
  production build. Existing real-Axum transport tests and component interactions
  retain distinct UI feedback and coalescing coverage.
- `devenv shell -- just api-sqlx-prepare`: passed; checked metadata was unchanged.
- `devenv shell -- just deployment-check`: passed.
- API and Dashboard local OCI exports both contain linux/amd64 and linux/arm64
  runtime images. API index:
  `sha256:24b18a89d6e8ec9e44ab155c8e1e60a47bfa0649b9f92c80efece152ac4a281c`.
  Dashboard index:
  `sha256:126f39f2bd54798bfbcf0bbba72b6ff0fa2ce3c4266eac0928e486d18820ccf4`.
- The original Dashboard ARM build stalled in emulated Vite SSR chunk rendering
  and was cancelled after several minutes without progress. Moving only the
  builder to `BUILDPLATFORM` made the same multi-platform recipe pass. Both final
  runtime stages still use their target architecture. The exported `.output`
  artifact was inspected for native addons/ELF binaries before sharing it across
  architectures; none were present.
- `just dashboard-docker-load` loads the local OCI export without publishing it.
  The existing amd64 `dashboard-docker-smoke` passed: production SSR, CSS/JS assets
  and private configuration handling. An additional ARM-emulated smoke could not
  complete because the local Docker daemon reported an unexpectedly missing
  container RWLayer during `docker cp`; ARM container execution is not claimed.
- The full, unskipped pre-commit `just check` gate must pass before integration;
  the completion comment records its final result.

Existing wall-switch/IR regressions passed through their established automated
provider seams. No wall-switch actuation, physical IR acknowledgment, physical
iPad/Safari check or live internet-isolation test is claimed by this ticket.

## Handoff to #211

Private launchers, raw profiles, database files, read-only snapshots, browser
screenshots and the boolean report are outside the repository in the established
private local evidence directory. The agent handoff records their exact paths;
never copy their credentials, device identity or raw profile contents into issues
or public artifacts. Local build/check logs and sanitized OCI inspection are kept
with the execution handoff. No duplicate automated tests were added merely to
restate already-covered contracts; the actual Docker build was the regression
check for the discovered build issue.

At completion of this local ticket, #211 still needed to verify the deployed API container's host BlueZ/system-D-Bus access,
UID/policy permissions, actual Pi adapter, deployed Dashboard→API→device flow,
unattended routine and restart/reconnect recovery. A native development-host probe
or local multi-platform image build does not establish these production facts.
The optional effects work in #210 is independent of this completed core gate.

These production gates have since passed in #211; see
[production verification](backlight-production-verification.md). The native Pi
release also establishes actual arm64 runtime execution after the local emulated
smoke limitation recorded above.
