# Local Backlight control

The `/backlight` Dashboard page is linked from Controls. The authenticated Axum
capability owns the main SQLite profile and intended selection, and serializes
complete GATT operations. Existing wall-switch and IR endpoints are independent.

Configure `BACKLIGHT_BLUETOOTH_ADDRESS` privately on the API host. The host needs
BlueZ (`bluetoothd`), a powered Bluetooth adapter and system D-Bus access. The Rust
transport uses BlueR's `bluetoothd` feature only. Its vendored D-Bus library builds
with the existing amd64/arm64 C cross compilers and adds no runtime shared-library
dependency. The API never starts another Bluetooth daemon.

For Docker, `api/compose.bluetooth.yml` prepares an opt-in system-bus socket mount
and bus address. It retains the existing unprivileged runtime UID, without
`privileged`, host networking, extra capabilities or a USB device mount. The host
D-Bus policy must permit this UID to send to `org.bluez`; use the host's existing
BlueZ policy where possible. Restrict any additional host policy to that UID and
BlueZ destination, and keep the socket unavailable to the Dashboard and MCP.
Production container permissions and Pi hardware verification belong to #211.

Capture the current complete TV Sync response and brightness with
`POST /api/backlight/profile {"source":"capture"}` while TV Sync is active.
A static mode or invalid checksum cannot replace a valid profile. The controller's
readable TV response and brightness are stored privately in the main database;
restoration rewrites those saved bytes with the command header/checksum and
restores the saved brightness. White balance, black-screen settings and camera
geometry are never changed by these commands. This is not a complete camera
calibration export.

A previously verified private probe backup can be recovered by setting
`BACKLIGHT_PROFILE_BACKUP` to its local path and using `{"source":"backup"}`.
For a container, mount that individual verified backup read-only and set its
container path. Keep the backup, database, device identity and raw GATT evidence
outside Git and public issue attachments. Recovery verifies the backup identity against the configured controller, then validates schema, complete
packet checksums, video mode and brightness; configure only the backup verified
for the current household controller.

`GET /api/backlight` returns sanitized cached status. `POST
/api/backlight/refresh` reads power, brightness and mode. `PUT
/api/backlight/selection {"mode":"off"}` and `{"mode":"tv-sync"}` persist the
intention before writing. A manual color uses, for example,
`{"mode":"color","rgb":{"red":255,"green":0,"blue":0},"brightness":20}`.
RGB channels are integers from 0 to 255; brightness commands are integers from
0 to 100. These are controller values, not calibrated intensity measurements.
`{"mode":"tv-sync","brightness":30}` adjusts TV Sync manually without changing
its saved profile. Selecting `{"mode":"tv-sync"}` restores the original saved
brightness. All endpoints use the existing API-key middleware;
Dashboard calls use its private server transport. A saved intention, a completed
GATT write and a readable observation are separate facts. The basic mode response
cannot prove exact static RGB. Observations become stale after 30 seconds or a
failed read; a failed write keeps its selected intention and reports failure.
Each device exchange has a 30-second bound; first-time profile capture can add
one exchange before a selection. Off remains available with incomplete setup.

Dashboard adjustments coalesce after
200 ms and retain only the latest pending edit. Across clients, selections are
persisted before waiting on Bluetooth; only the latest pending target reaches the
controller after the current complete GATT operation finishes. Accepted work
continues if its requesting client disconnects. Refresh and profile operations
share the same device-operation lock. Cached reads never issue device writes;
external changes remain untouched during a healthy connection until another explicit
command or scheduled transition. Startup and reconnect deliberately restore the
current OpenHome target.

Bedtime Mode starts disabled, retaining 23:00–00:00 Europe/Copenhagen and its own
20% red brightness. `PATCH /api/backlight/bedtime {"enabled":true}` clears manual
overrides and applies Automatic immediately. Start is inclusive; End is exclusive.
Red powers on even after Off; End restores the saved powered-on TV Sync profile.
Disabling clears overrides, restores TV Sync and retains the window settings.
`POST /api/backlight/automatic` clears overrides immediately, selecting the current
routine target (TV Sync while disabled). Manual selections during an enabled
routine expire at its next actual transition; while disabled they have no expiry.
Status exposes `bedtime`, `control_source`, `override_expires_at` and
`next_transition`. Settings, override selection/expiry and intended target commit
atomically to SQLite. The API's background task evaluates current wall time,
independently of Dashboard activity, and writes once at meaningful transitions.
The LED form saves Start, End and independent red brightness through the same
PATCH operation, for example `{"start":"22:30","end":"02:00","brightness":30}`.
Omitted fields retain their values; nulls, equal times and invalid values are
rejected without changing settings or the intended target. Times use exact
`HH:MM` from 00:00 to 23:59; red brightness is an integer from 0 to 100.
Edits apply the current Automatic target immediately. An unexpired manual
selection stays selected and receives the edited window's next actual transition
as its expiry. Disabled edits retain an indefinite manual selection and saved
window settings. Supplying Enabled explicitly clears the override as above.

Every actual minute is evaluated against Copenhagen wall time. During the spring
jump, a partially skipped window starts or ends at the first real minute whose
membership changes; a wholly skipped window has no transition that day. During
the autumn repeat, a window contained within the repeated hour runs twice.
Rollback itself can leave a window and the second occurrence can reenter it;
a window covering the whole repeated hour stays active throughout. Override
expiry is the next actual change, including a rollback change, rather than an
arbitrarily chosen occurrence of an ambiguous local boundary.

Startup validates the saved profile, retains unexpired manual selections and
removes expired overrides against the current Copenhagen window. The background
worker applies only the current target; missed transitions and superseded commands
are never replayed. After waiting for connection/discovery, the transport checks the
current revision and window deadline again before starting its write batch.
Expired or superseded batches send no obsolete target. Profile recovery does not capture or replace a valid saved
profile from reconnect observations. Missing or invalid profiles report incomplete
setup; non-Off recovery waits for a valid saved profile.

Every ten seconds the worker reads device state to detect an unavailable adapter
or link. A newly established Bluetooth link also triggers recovery even if its
read succeeds. Healthy reads do not repeat target writes. Failed operations retain
the last observation as stale and retry only the latest target with exponential
backoff from one second, capped at thirty seconds (plus the bounded exchange time).
A new explicit selection attempts immediately and supersedes queued recovery work.
The status contract distinguishes `recovery` pending/retrying/recovered/profile-required,
command pending/failed/acknowledged, and fresh versus stale observations. Recovered
means the current write was accepted and readable state returned; it does not
establish exact visible RGB. Persistence failures retain the due transition and
retry saving it before hardware work. Initial settings do not activate a
production schedule.
