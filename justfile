default:
    @just --list

# Show this help grouped by area
groups:
    @just --list --list-heading '' --justfile '{{justfile()}}'

expodir := 'mobile-expo'

# Build the OpenHome CLI
[group('cli')]
[working-directory: 'cli']
cli-build:
    cargo build

# Run the OpenHome CLI (just cli-run health)
[group('cli')]
[working-directory: 'cli']
cli-run *args:
    cargo run -- {{args}}

# Test the OpenHome CLI
[group('cli')]
[working-directory: 'cli']
cli-test:
    cargo test

# Format the OpenHome CLI
[group('cli')]
[working-directory: 'cli']
cli-format:
    cargo fmt

# Lint the OpenHome CLI (clippy denies warnings)
[group('cli')]
[working-directory: 'cli']
cli-lint:
    cargo clippy -- -D warnings

# Run the Axum API server
[group('api')]
[working-directory: 'api']
run:
    cargo run

# Run the full API test suite
[group('api')]
[working-directory: 'api']
test:
    cargo test

# Run one API test by exact name (just test-one my_test)
[group('api')]
[working-directory: 'api']
test-one name:
    cargo test {{name}} -- --exact

# Run one API integration test file (just test-integration my_integration)
[group('api')]
[working-directory: 'api']
test-integration name:
    cargo test --test {{name}}

# Format then check the API
[group('api')]
[working-directory: 'api']
fmt:
    cargo fmt
    cargo clippy

# Lint the API (clippy denies warnings)
[group('api')]
[working-directory: 'api']
lint:
    cargo clippy -- -D warnings

# Test, format, and lint the API in one pass
[group('api')]
[working-directory: 'api']
go:
    cargo test
    cargo fmt
    cargo clippy

# Install Expo client dependencies
[group('expo')]
[working-directory: 'mobile-expo']
expo-install:
    bun install

# Start the Expo dev server
[group('expo')]
[working-directory: 'mobile-expo']
expo-start:
    bun start

# Build and install the Expo app on a connected Android device (release variant)
[group('expo')]
[working-directory: 'mobile-expo']
expo-android:
    #!/usr/bin/env bash
    set -euo pipefail
    export ANDROID_HOME="$(android info | sed -n 's/^sdk: //p')"
    bunx expo run:android --variant release --no-bundler

# Typecheck the Expo client
[group('expo')]
[working-directory: 'mobile-expo']
expo-typecheck:
    bun run typecheck

# Lint the Expo client
[group('expo')]
[working-directory: 'mobile-expo']
expo-lint:
    bun run lint

# Format the Expo client with oxfmt
[group('expo')]
[working-directory: 'mobile-expo']
expo-format:
    bun run format

# Run Expo client tests, optionally filtered (e.g. just expo-test adguard; filters are filename substrings)
[group('expo')]
expo-test filter='':
    @cd {{expodir}} && bunx vitest run{{ if filter == '' { '' } else { ' ' + filter } }}

# Full Expo gate: typecheck + tests + Android export
[group('expo')]
[working-directory: 'mobile-expo']
expo-check:
    bun run check

# Compile the ESP32 IR remote firmware
[group('firmware')]
[working-directory: 'firmware/ir-remote']
ir-build:
    pio run

# Upload the IR remote firmware to the automatically detected serial port
[group('firmware')]
[working-directory: 'firmware/ir-remote']
ir-upload:
    pio run --target upload

# Alias for ir-upload
ir-flash: ir-upload

# Open the IR remote's 115200-baud serial monitor
[group('firmware')]
[working-directory: 'firmware/ir-remote']
ir-monitor:
    pio device monitor

# Upload the IR remote firmware, then open its serial monitor
ir-run: ir-upload ir-monitor

# List serial devices visible to PlatformIO
[group('firmware')]
[working-directory: 'firmware/ir-remote']
ir-devices:
    pio device list

# Remove IR remote build artifacts
[group('firmware')]
[working-directory: 'firmware/ir-remote']
ir-clean:
    pio run --target clean

switchbot-port := env_var_or_default('UPLOAD_PORT', '/dev/ttyUSB0')
switchbot-core-dir := '.platformio'

# Compile the ESP32 SwitchBot firmware
[group('firmware')]
[working-directory: 'firmware/switchbot']
switchbot-build:
    PLATFORMIO_CORE_DIR={{switchbot-core-dir}} pio run

# Upload the SwitchBot firmware to UPLOAD_PORT (default /dev/ttyUSB0)
[group('firmware')]
[working-directory: 'firmware/switchbot']
switchbot-flash:
    PLATFORMIO_CORE_DIR={{switchbot-core-dir}} pio run --target upload --upload-port {{switchbot-port}}

# Open the SwitchBot serial monitor on UPLOAD_PORT
[group('firmware')]
[working-directory: 'firmware/switchbot']
switchbot-monitor:
    PLATFORMIO_CORE_DIR={{switchbot-core-dir}} pio device monitor --port {{switchbot-port}}

# Remove SwitchBot build artifacts
[group('firmware')]
[working-directory: 'firmware/switchbot']
switchbot-clean:
    PLATFORMIO_CORE_DIR={{switchbot-core-dir}} pio run --target clean
