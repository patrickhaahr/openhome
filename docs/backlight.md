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

Manual selections have no hidden expiry. Dashboard adjustments coalesce after
200 ms and retain only the latest pending edit. Across clients, selections are
persisted before waiting on Bluetooth; only the latest pending target reaches the
controller after the current complete GATT operation finishes. Accepted work
continues if its requesting client disconnects. Refresh and profile operations
share the same device-operation lock. Cached reads never issue device writes;
external changes remain untouched until another explicit command.

No schedule is activated yet. Automatic/routine behavior and reconnect recovery
are delivered by the subsequent epic tickets.
