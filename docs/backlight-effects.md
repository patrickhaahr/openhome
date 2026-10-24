# Local H605C effects

The LED page's Effects playground installs five device-native scenes. Select one
with a tap, or send an authenticated request such as
`PUT /api/backlight/selection {"mode":"effect","effect":"aurora","brightness":30}`.
The status response includes the installed `effects` names/descriptions. Unknown
identifiers, extra fields and invalid brightness are rejected. No account, Wi-Fi,
scene download, cloud request or server-side animation loop is needed at runtime.

| Stable identifier | Name | Catalog scene ID | Scene code | H605C payload |
| --- | --- | --- | --- | --- |
| `forest` | Forest | 130 | 212 | H605C specialEffect, 103 bytes |
| `aurora` | Aurora | 135 | 215 | H605C specialEffect, 70 bytes |
| `rainbow` | Rainbow | 144 | 10187 | H605C catalog default, 43 bytes |
| `fire` | Fire | 145 | 216 | H605C specialEffect, 70 bytes |
| `party` | Party | 159 | 226 | H605C specialEffect, 136 bytes |

The offline assortment was taken from the manufacturer's
[H605C catalog](https://app2.govee.com/appsku/v1/light-effect-libraries?sku=H605C),
retrieved 23 October 2026. Model-specific `specialEffect` parameters take priority;
Rainbow uses its nonempty H605C catalog variant. The fixed 20-byte frames in
[`effects.rs`](../api/src/services/backlight/effects.rs) use the ptReal layout
researched in [Govee Lights' scene encoder at cb590bb](https://github.com/Jaano/govee_lights/blob/cb590bb432c278f75a2f699378ee8daa16e447e4/custom_components/govee_lights/govee.py):
19-byte bodies with an A3 header, sequence/final markers and XOR checksum,
followed by the little-endian scene activation, power and selected brightness.
This is a fixed local assortment for the tested H605C firmware, not compatibility
with other Govee models or arbitrary future firmware.

Effects are ordinary durable Backlight Overrides. Brightness adjustments keep the
selected effect, while Off, TV Sync and Automatic exit it. An enabled Bedtime Mode
transition expires the override and takes over, including red at Start and saved
TV Sync at End. An API restart or Bluetooth recovery reconstructs the current
selection from the same SQLite state. The saved TV Sync profile and its brightness
are independent of effect brightness; effect selection never edits calibration,
white balance or black-screen behavior.

Each upload uses the existing operation lock. The transport checks the current
revision and routine deadline before every frame; an obsolete transfer stops
before its remaining frames/activation, and a new scene starts with its own A3
header. Uploads and readback operations cannot interleave. A frame already handed
to BlueZ cannot be recalled. Readback establishes power, basic mode and brightness,
not the scene identity, visible colors or animation. The UI therefore shows
Requested effect separately from Observed mode (currently Other for native scenes).

## Development-host evidence

All five installed scenes were exercised through the actual authenticated Axum
API and the development host's native Bluetooth adapter. Each returned accepted
writes, fresh powered-on native-mode readback and the requested brightness.
Switching, rapid supersession, actual API restart, target-only Bluetooth outage,
latest-effect recovery, explicit exits, real wall-clock red takeover and end-of-window
TV Sync restoration were checked. The saved private TV Sync profile remained
byte-for-byte unchanged. This is technical evidence; the user explicitly waived
human LED/camera confirmation and instructed that visible effects be assumed to
work. No human observation is claimed.

The empty Rainbow/code 22 variant was excluded before device testing. Additional
Aurora, Fire and Party variants, music modes, static segment palettes and catalogs
for other models were not tested and are not exposed. There were no failing
installed candidates hidden by the UI.

Automated coverage uses the authenticated API with temporary main SQLite and the
existing device-operation boundary: durable effect selection/reconstruction,
validation, known H605C frame boundaries, partial-upload cancellation, latest-target
serialization and exact saved TV Sync restoration. Dashboard interaction coverage
checks one-tap selection and requested-versus-observed state. Synthetic desktop and
phone screenshots are in [the redesign gallery](../dashboard/docs/redesign/README.md).
They use the real production component with invented status/replies in the
synthetic Dashboard shell, without household settings or device identifiers.

Production Docker/Pi execution remains the final release ticket #211. This ticket
does not deploy an intermediate slice or establish physical iPad/Safari behavior.
