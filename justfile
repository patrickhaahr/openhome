default:
    @just --list

[working-directory: "api"]
run:
    cargo run

[working-directory: "api"]
test:
    cargo test

[working-directory: "api"]
test-one name:
    cargo test {{name}} -- --exact

[working-directory: "api"]
test-integration name:
    cargo test --test {{name}}

[working-directory: "api"]
fmt:
    cargo fmt
    cargo clippy

[working-directory: "api"]
lint:
    cargo clippy -- -D warnings

[working-directory: "api"]
go:
    cargo test
    cargo fmt
    cargo clippy

[working-directory: "mobile-native"]
android-build:
    #!/run/current-system/sw/bin/nu
    ^direnv exec /home/ph/dev/openhome /run/current-system/sw/bin/bash -lc 'set -euo pipefail; cd /home/ph/dev/openhome/mobile-native; sdk_root=$(android info | sed -n "s/^sdk: //p"); original=$(mktemp); trap "cp \"$original\" local.properties; rm -f \"$original\"" EXIT; cp local.properties "$original"; printf "sdk.dir=%s\n" "$sdk_root" > local.properties; ./gradlew :app:assembleDebug'

[working-directory: "mobile-native"]
android-run: android-build
    #!/run/current-system/sw/bin/nu
    ^direnv exec /home/ph/dev/openhome /run/current-system/sw/bin/bash -lc 'set -euo pipefail; cd /home/ph/dev/openhome/mobile-native; sdk_root=$(android info | sed -n "s/^sdk: //p"); original=$(mktemp); trap "cp \"$original\" local.properties; rm -f \"$original\"" EXIT; cp local.properties "$original"; printf "sdk.dir=%s\n" "$sdk_root" > local.properties; android run --apks "app/build/outputs/apk/debug/app-debug.apk" --activity "com.example.openhome.MainActivity"'

[working-directory: "mobile-native"]
android-lint:
    #!/run/current-system/sw/bin/nu
    ^direnv exec /home/ph/dev/openhome /run/current-system/sw/bin/bash -lc 'set -euo pipefail; cd /home/ph/dev/openhome/mobile-native; sdk_root=$(android info | sed -n "s/^sdk: //p"); original=$(mktemp); trap "cp \"$original\" local.properties; rm -f \"$original\"" EXIT; cp local.properties "$original"; printf "sdk.dir=%s\n" "$sdk_root" > local.properties; ./gradlew ktlintCheck :app:lint'

[working-directory: "mobile-native"]
android-test:
    #!/run/current-system/sw/bin/nu
    ^direnv exec /home/ph/dev/openhome /run/current-system/sw/bin/bash -lc 'set -euo pipefail; cd /home/ph/dev/openhome/mobile-native; sdk_root=$(android info | sed -n "s/^sdk: //p"); original=$(mktemp); trap "cp \"$original\" local.properties; rm -f \"$original\"" EXIT; cp local.properties "$original"; printf "sdk.dir=%s\n" "$sdk_root" > local.properties; ./gradlew :app:testDebugUnitTest'

[working-directory: "mobile-native"]
android-test-ui:
    #!/run/current-system/sw/bin/nu
    ^direnv exec /home/ph/dev/openhome /run/current-system/sw/bin/bash -lc 'set -euo pipefail; cd /home/ph/dev/openhome/mobile-native; sdk_root=$(android info | sed -n "s/^sdk: //p"); original=$(mktemp); trap "cp \"$original\" local.properties; rm -f \"$original\"" EXIT; cp local.properties "$original"; printf "sdk.dir=%s\n" "$sdk_root" > local.properties; ./gradlew :app:connectedDebugAndroidTest'

# Compile the ESP32 IR remote firmware.
[working-directory: "firmware/ir-remote"]
ir-build:
    pio run

# Upload the IR remote firmware to the automatically detected serial port.
[working-directory: "firmware/ir-remote"]
ir-upload:
    pio run --target upload

# Alias for ir-upload.
ir-flash: ir-upload

# Open the IR remote's 115200-baud serial monitor.
[working-directory: "firmware/ir-remote"]
ir-monitor:
    pio device monitor

# Upload the IR remote firmware, then open its serial monitor.
ir-run: ir-upload ir-monitor

# List serial devices visible to PlatformIO.
[working-directory: "firmware/ir-remote"]
ir-devices:
    pio device list

# Remove IR remote build artifacts.
[working-directory: "firmware/ir-remote"]
ir-clean:
    pio run --target clean

switchbot-port := env_var_or_default("UPLOAD_PORT", "/dev/ttyUSB0")
switchbot-core-dir := ".platformio"

# Compile the ESP32 SwitchBot firmware.
[working-directory: "firmware/switchbot"]
switchbot-build:
    PLATFORMIO_CORE_DIR={{switchbot-core-dir}} pio run

# Upload the SwitchBot firmware.
[working-directory: "firmware/switchbot"]
switchbot-flash:
    PLATFORMIO_CORE_DIR={{switchbot-core-dir}} pio run --target upload --upload-port {{switchbot-port}}

# Open the SwitchBot serial monitor.
[working-directory: "firmware/switchbot"]
switchbot-monitor:
    PLATFORMIO_CORE_DIR={{switchbot-core-dir}} pio device monitor --port {{switchbot-port}}

# Remove SwitchBot build artifacts.
[working-directory: "firmware/switchbot"]
switchbot-clean:
    PLATFORMIO_CORE_DIR={{switchbot-core-dir}} pio run --target clean
