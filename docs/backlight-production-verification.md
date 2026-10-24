# Backlight production verification

Production acceptance for [#211](https://github.com/patrickhaahr/openhome/issues/211),
following the development-host verification in `backlight-local-verification.md`
and the native scenes in `backlight-effects.md`.

## Release and container boundary

The repository release workflow built and published API **0.6.3**, MCP **0.3.3**
and Dashboard **0.2.4** on the development host, for amd64 and arm64. The Pi ran
all three images natively as arm64. The workflow verified immutable release-image
identities, healthy containers, zero restarts, authenticated API/MCP reads,
unauthenticated rejection, HTTPS Dashboard and 21 CSS/JS assets. The local
production-image smoke also verified rendered HTML/assets and private server
configuration. A final verification repeated these checks after the acceptance
experiments.

Bluetooth is an explicit private `COMPOSE_FILE` opt-in. Only the API receives the
host system-bus socket and read-only private profile directory. Its actual process
ran as UID 10001 with zero effective capabilities, without a privileged container,
added capabilities, host networking or USB passthrough. The Pi's existing default
BlueZ policy was sufficient. The first real container test exposed a missing host
identity for that numeric UID: same-UID D-Bus authentication failed before a
locked, no-home service identity was registered, then actual API Bluetooth writes
and readback succeeded. Deployment now preserves this prerequisite without
broadening D-Bus policy.

Before activation, both main and NOOP SQLite databases were backed up through
SQLite's online backup API and passed `quick_check`. Previous image identities,
Compose files and private environment were retained. The validated private TV Sync
backup was transferred read-only; no raw profiles, credentials or controller
identifiers are included here. Release configuration is installed only after its
previous configuration has been preserved. The development-host link was released
before Pi ownership.

The final native acceptance also exposed a keepalive gap: the original ten-second
query cadence allowed the controller to drop its link after about twenty seconds,
which correctly triggered recovery but repeatedly replaced external selections.
A device-only control kept the connection and external brightness with two-second
power queries. The same API external-selection check failed before the cadence
change and passed on the development host afterward, then passed on the corrected
Pi release. The existing worker now uses a two-second query interval.

## Actual deployed flow

The production LED page used its server transport and authenticated Axum API to
control the real controller through the Pi's BlueZ daemon:

- Off returned power-off readback. RGB red/green/blue returned matching accepted
  requests and readable color mode; brightness 32 returned brightness readback.
  TV Sync restored the saved profile and its independent original brightness.
- All five native scenes were selected through the deployed page. Each completed
  its requested upload and returned powered-on native basic-mode readback. Exact
  effect identity and static RGB are not readable and are not claimed as such.
- The form rejected equal times, saved a custom window/red brightness, enabled
  Automatic, displayed Manual with an expiry, and cleared the override through
  Automatic. A real target-only outage was shown as unreachable/stale.
- Existing IR status and volume-down/volume-up, and wall-switch Off/On, reached
  their actual integration bridges and returned acknowledgments. These endpoints
  do not establish physical speaker volume or wall-light state.

The verification browser was closed for real wall-clock routine experiments; no
stored clock manipulation or schedule simulation was used. A container restart
retained an unexpired Off override. Its next Start powered on red at 31%, and End
restored TV Sync and saved brightness. A native Fire scene was overridden at a
second Start, at 37% red. A separate active-TV-Sync window used 42% red and again
returned to the original saved brightness.

A target-specific BlueZ Blocked/Disconnect outage spanned End and a real stopped
API interval. New selections remained durable while unreachable. Startup after
their expiry discarded the expired override and missed red interval; unblocking
recovered the current Automatic TV Sync target. The final current-target/readback
result complements the deterministic transport tests for exact obsolete-write
suppression; the private D-Bus trace is not an over-the-air capture.

An independent host selected static blue and brightness 18 on the existing link
while the API still intended TV Sync. The API observed this external selection
and left it untouched across 65.6 seconds of background reads. Private D-Bus
WriteValue/Connected tracing counted 66 status queries, zero lighting-target
writes and zero disconnects during that hold. Explicit Automatic then restored
the saved brightness. A fresh production capture and independent before/after read-only snapshots matched the
saved profile and all seven supported readable replies: power, brightness, full
video response, firmware, white balance, whole-screen brightness and black-screen
behavior. The private operational handoff retains exact timestamps, request/status
evidence, backups and rollback image identities.

## Final state and limits

Bedtime Mode is **enabled**, daily **23:00–00:00 Europe/Copenhagen**, with **20%
red**. Control source is **Automatic**, without a manual override. Outside the
window the controller is powered on in saved TV Sync with its original brightness
and profile. The target is unblocked; the development host retains no connection.

Visual LED/camera confirmation was explicitly waived by the user; no human
observation or camera-image responsiveness is claimed. Readable settings are not
a geometric camera-calibration backup. Edge brightness is unsupported. These were
short real windows and a bounded outage, not overnight stability evidence.
Physical iPad/Safari and live internet-isolation acceptance were not performed.
