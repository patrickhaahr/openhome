# TV backlight plan

Status: agreed design published in GitHub epic #203. Tickets #204–#210 cover
development and local verification; final ticket #211 covers Docker deployment
and production end-to-end testing. No product implementation, deployment or
schedule activation has taken place.

## Final design

### Capability and device access

Develop and test on the current development host, whose Bluetooth adapter was
used in the initial successful hardware tests. All coding, debugging, automated
checks, local hardware experiments and release image builds happen here. The
Raspberry Pi is the production Docker target after the feature is complete;
do not use it as a development/build host or deploy intermediate slices there.

In production, use direct Bluetooth from the existing Pi for the single H605C. Axum owns its
device integration, durable routine, overrides and status; dashboard requests
continue through the authenticated API. Existing wall-switch lights and the IR
ESP32 remain separate. The Govee retains its existing camera processing,
calibration and black-screen behavior. No TV-power telemetry, cloud integration,
camera feed, ESP32 BLE implementation or generic automation engine is required.

### Bedtime Mode

The LED page provides Enabled, Start, End and red-brightness settings. Initial
times are 23:00–00:00 daily in Europe/Copenhagen; red brightness starts at 20%
and is separate from TV Sync brightness. The interval includes its start and
excludes its end, and can span midnight. Equal start/end is invalid rather than
an implicit all-day window.

When enabled and Automatic, select powered-on red inside the window and TV Sync
outside it, including at midnight while watching. The bedtime transition powers
on the red cue even if the strip was previously off. Returning to TV Sync
restores the captured video profile and its brightness; it does not power off
the controller or change its response to a black picture.

Enabling Bedtime Mode clears the current override and immediately resumes
Automatic under the saved window. Disabling it clears the override, restores
TV Sync and stops scheduled changes. Disabled window settings remain saved.

### Manual control and editing

The dedicated LED page at `/backlight` is linked from Controls, without a new
main-navigation item. It exposes Off, TV Sync, RGB color selection, brightness,
Automatic and Bedtime Mode settings. Coalesce slider interaction before sending
commands. Show the control source and any override expiry.

Manual selections override an enabled routine until its next transition.
Automatic clears the override and applies the current routine selection. With
Bedtime Mode disabled there is no scheduled expiry: a manual selection remains
until another explicit selection, Automatic, or enabling Bedtime Mode.

Saving times or red brightness immediately reevaluates the routine when
Automatic. During an override, retain the manual selection and recalculate its
expiry against the edited window. Commands from other clients serialize through
the same backend controller.

### Persistence, recovery and observation

Persist settings, overrides and their expiry, the latest intended target and
the validated video profile in the existing main SQLite database. Evaluate the
current window after startup/reconnect; discard expired overrides and obsolete
actions. An unexpired override remains the current target. Retry only the latest
target, with bounded backoff; individual GATT operations must not interleave.

Physical-button or Govee-app changes are left alone during uninterrupted
operation until a scheduled transition or explicit OpenHome command. Startup or
reconnect is an accepted exception: reapply OpenHome's current target. Routine
reevaluation must not cause continuous reassertion between these events.

Separate intended settings, write acceptance and device-confirmed observations.
Supported queries confirm power, brightness and mode, but the tested basic mode
reply does not confirm exact RGB. Stale/unreachable state stays explicit.

Missing/invalid TV Sync settings keep setup incomplete, with Off available.
Recover from a verified private backup or capture while the device is in TV
Sync. Persist the profile before first leaving TV Sync, and do not replace it
with a static-mode response or invent default camera settings.

### Implementation and acceptance

Keep handlers thin around a focused backlight module with router-local shared
state, a pure routine evaluator and one serialized device worker. Add a main-DB
migration and checked SQLx queries. Implement and verify native Bluetooth on
the development host and prepare Docker/cross-build dependencies locally. Final
ticket #211 verifies the deployed API container's Pi host BlueZ/D-Bus access
and runtime permissions. Keep MACs and private profile data in deployment
configuration/storage, outside public code and fixtures.

Acceptance covers the 23:00 red / 00:00 TV Sync cycle, midnight-spanning edited
windows, Copenhagen DST, enabling/disabling, override expiry and edits, restart/
reconnect recovery, missing profile, stale status, and latest-target retry
behavior. Verify physical color and camera restoration from the API running on
the development host, with the dashboard closed and after an API restart. Ticket
#209 is integrated local verification; #210 adds locally tested effects. Final
ticket #211 follows both and owns Docker deployment plus production E2E on the
Pi, including its real container Bluetooth access and long-running behavior.
Run the changed areas' project gates through `devenv shell -- just ...` and
refresh SQLx metadata for new checked queries. Deployment remains a separate
operation governed by the repository deployment workflow.

## Stated requirements and evidence

- Add manual dashboard controls for the Govee Envisual T2 / H605C.
- Keep its existing camera-responsive TV lighting available.
- Daily, use TV Sync outside the bedtime window; switch to red at 23:00 and
  restore TV Sync at 00:00, including while the TV is being watched. This
  explicitly replaces the earlier midnight power-off proposal. Times are editable.
- Bedtime red uses its own brightness, initially 20%, independently of TV Sync.
- Prefer direct Bluetooth control from the nearby Raspberry Pi. Pi discovery,
  RGB cycling and return to the saved video settings were physically tested and
  confirmed by the user. Leave the existing IR ESP32's firmware unchanged.
- The device was not associated with the household router during inspection;
  a device-specific preventive internet block was installed in the network repo.
- Readable video settings, white balance, brightness and black-screen behavior
  have private backups on both Pi and development host. The backups do not
  include camera geometric-calibration coordinates.
- Clients continue to call Axum; credentials, device communication and automation
  belong behind that existing interface.

## Current repository facts

- `CONTEXT.md` is the existing domain glossary. Its Light Command means the
  servo wall-switch command; backlight control must not silently redefine it.
- The API container has no declared host BlueZ/D-Bus access. Host Bluetooth tests
  succeeded, but the final container access and cross-build remain to be proven.
- The RSS background task is an elapsed-time refresher, not an existing durable
  wall-clock automation engine.
- TV power is not observed by the current IR integration. Bedtime overriding
  viewing removes TV-power telemetry as a prerequisite for the requested routine.

## Design tree status

All interview branches are settled: device access and scope, Bedtime Mode
window, red brightness and power-on, manual overrides, editing, recovery,
external controls, missing-profile handling, dashboard placement and vocabulary.
The implementation details and acceptance checks above define the agreed
design. Development is local; production deployment belongs to final #211.

## Round one decisions

- Q1: Daily routine in Europe/Copenhagen, editable bedtime 23:00 and midnight
  transition 00:00. Q7 resolves midnight as return to TV Sync.
- Q2: Bedtime red has its own brightness, initially 20%, independently of TV Sync.
- Q3: The user reports leaving the controller powered on in its responsive
  camera mode, with visible output responding to the TV picture. Q7 confirms
  returning to that state after midnight. Visible darkness is not necessarily
  controller power-off or motion detection.
- Q4: A manual selection lasts until the next scheduled transition. Automatic
  clears it immediately. Show its expiry on the dashboard.
- Q5: After API restart or Bluetooth recovery, select the setting appropriate
  now and discard obsolete historical actions. Persist the routine and expiry.
- Q6: Use backlight in code/domain names; LED is acceptable in dashboard copy.
  Keep the existing wall-switch Light Command meaning. Record Backlight and
  TV Sync terminology in the existing `CONTEXT.md` glossary.

## Round two decisions

- Q7: At 00:00 restore TV Sync even if the TV is still playing. The normal mode
  is powered-on camera responsiveness, with the device's existing black-screen
  behavior preserved. No hard power-off or morning-resume phase is required.
- Q8: At 23:00 turn on the bedtime red cue even if the controller was manually
  powered off or its camera response was dark. The user normally leaves it on.
- Q9: First-release dashboard controls: Off, TV Sync, RGB color selection,
  brightness, Automatic and routine settings. Camera geometric calibration and
  black-screen settings remain untouched.
- Q10: A change made through physical buttons or the Govee app is left alone
  until the next scheduled transition or explicit OpenHome command. The user
  does not expect to use those other controls. Q11 defines restart/reconnect
  as an explicit exception: OpenHome reapplies its current target.

## Round three decisions

- Q11: Restart/reconnect reapplies the current OpenHome target, including any
  unexpired persisted Backlight Override, even if an external control previously
  changed the light. Q10's leave-alone behavior covers uninterrupted operation.
- Q12: Saving routine times or red brightness immediately evaluates and applies
  the updated routine when Automatic is active. An active manual override stays
  selected, with expiry recalculated to the edited routine's next transition.
  The user describes the routine as a night/bed mode whose configured time
  window shows red. Use Bedtime Mode for that concept.
- Q13: Missing/invalid saved TV Sync settings keep setup incomplete. Recover a
  valid profile from the device while in TV Sync or from the verified private
  backup; Off remains available. Do not invent default camera settings.
- Q14: Dedicated LED page at `/backlight`, opened from Controls, without a new
  main-navigation item initially.

## Round four decision

- Q15: Bedtime Mode has Enabled, Start, End and red brightness. Defaults are
  23:00–00:00 and 20%. Enabling resumes Automatic immediately, selecting red
  inside the window and TV Sync outside it. Disabling restores TV Sync and
  stops scheduled changes. Manual selections override an enabled routine until
  its next transition.

The interview is complete and the design was approved for publication. The
published epic and tickets are the durable implementation handoff. Final #211
owns production rollout after local development and verification are complete.
