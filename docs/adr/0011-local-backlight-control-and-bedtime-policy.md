# Local backlight control and bedtime policy

Accepted design; published in epic #203 and implementation tickets #204–#211.

Develop, debug, test and build images on the current development host, where
the initial Bluetooth hardware tests succeeded. The Raspberry Pi runs the
completed Docker deployment. Integrated local verification is #209; the effects
playground is #210; final #211 owns deployment and production end-to-end testing
after both. Intermediate implementation slices do not develop on or deploy to
the Pi.

The H605C's local Bluetooth controls and exact video-profile restoration were
verified from the nearby Pi, while local Wi-Fi control has not been established.
Use a dedicated Axum-owned backlight capability with one serialized Bluetooth
worker and persistence in the existing main SQLite database. This keeps the
working IR ESP32 unchanged and avoids a Govee cloud dependency, at the cost of
explicit host BlueZ/D-Bus access and compatible dependencies in the API's
container and cross-build. Dashboard clients retain the existing Axum interface.

Bedtime Mode selects red during a configured daily Copenhagen time window and
TV Sync outside it. Midnight returns to powered-on camera responsiveness; it is
not a power-off operation. The Govee's existing camera and black-screen settings
remain responsible for picture response, so this routine does not require TV
power telemetry. Manual overrides, settings and captured video profiles survive
restarts; recovery applies the currently intended state rather than replaying
historical actions. Device observations remain distinct from intended settings
and write acknowledgments. Existing wall-switch Light Commands keep their
acknowledgment-only meaning.

Product behavior, confirmed interview decisions and acceptance criteria are in
[the backlight plan](../tv-backlight-plan.md); terminology is in
[`CONTEXT.md`](../../CONTEXT.md#backlight).
