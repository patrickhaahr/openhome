default:
    @just --list

# Full gate across repos
check: check-api check-mcp check-mobile check-firmware check-cli

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
    cargo clippy --all-targets --all-features --locked -- -D warnings

# Build the Hermes MCP adapter
[group('mcp')]
[working-directory: 'mcp']
mcp-build:
    cargo build

# Run the MCP adapter (needs OPENHOME_API_URL and API_KEY; see mcp/AGENTS.md)
[group('mcp')]
[working-directory: 'mcp']
mcp-run:
    cargo run

# Test the MCP adapter against the real API training routes
[group('mcp')]
[working-directory: 'mcp']
mcp-test:
    cargo test

# Format the MCP adapter
[group('mcp')]
[working-directory: 'mcp']
mcp-format:
    cargo fmt

# Lint the MCP adapter (clippy denies warnings)
[group('mcp')]
[working-directory: 'mcp']
mcp-lint:
    cargo clippy --all-targets --all-features --locked -- -D warnings

# Test, format, and lint the MCP adapter in one pass
[group('mcp')]
[working-directory: 'mcp']
mcp-check: mcp-format mcp-lint mcp-test

# Format the Axum API server
[group('api')]
[working-directory: 'api']
api-fmt:
    cargo fmt

# Lint the Axum API (clippy denies warnings)
[group('api')]
[working-directory: 'api']
api-lint:
    cargo clippy --all-targets --all-features --locked -- -D warnings

# Run the full API test suite
[group('api')]
[working-directory: 'api']
api-test:
    cargo test

# Run one API test by exact name (just api-test-one my_test)
[group('api')]
[working-directory: 'api']
api-test-one name:
    cargo test {{name}} -- --exact

# Run one API integration test file (just api-test-integration my_integration)
[group('api')]
[working-directory: 'api']
api-test-integration name:
    cargo test --test {{name}}

# Run the Axum API server
[group('api')]
[working-directory: 'api']
api-run:
    cargo run

# Format, lint, and test the API in one pass
[group('api')]
[working-directory: 'api']
api-check: api-fmt api-lint api-test

# Create a reversible NOOP push migration (just api-noop-migration-add receiver_state)
[group('api')]
[working-directory: 'api']
api-noop-migration-add name:
    sqlx migrate add -r --source noop_migrations {{name}}

# Apply NOOP push migrations to NOOP_DB_URL (default: noop.db next to DATABASE_URL)
[group('api')]
[working-directory: 'api']
api-noop-migrate:
    #!/usr/bin/env bash
    set -euo pipefail
    # Mirrors noop_push::resolve_db_url in src/services/noop_push.rs; keep the two in sync.
    url="${NOOP_DB_URL:-}"
    url="${url#"${url%%[![:space:]]*}"}"
    url="${url%"${url##*[![:space:]]}"}"
    if [ -z "$url" ]; then
        db="${DATABASE_URL:?DATABASE_URL must be set}"
        location="${db%%\?*}"
        query=""
        if [ "$location" != "$db" ]; then query="?${db#*\?}"; fi
        if [[ "$location" != sqlite:* ]]; then
            echo "cannot derive the NOOP push database location from DATABASE_URL; set NOOP_DB_URL explicitly" >&2
            exit 1
        fi
        path="${location#sqlite:}"
        dir=""
        file="$path"
        if [[ "$path" == */* ]]; then dir="${path%/*}/"; file="${path##*/}"; fi
        case "$file" in
            "" | ":memory:" | "noop.db")
                echo "cannot derive the NOOP push database location from DATABASE_URL; set NOOP_DB_URL explicitly" >&2
                exit 1
                ;;
        esac
        url="sqlite:${dir}noop.db${query}"
    fi
    echo "NOOP push database: $url"
    sqlx database create --database-url "$url"
    sqlx migrate run --source noop_migrations --database-url "$url"

# Build the API image for amd64 + arm64 (Raspberry Pi) locally
[group('api')]
[working-directory: 'api']
api-docker-build version:
    docker buildx build --builder openhome-publisher --platform linux/amd64,linux/arm64 -t patrickhaahr/openhome-api:{{version}} -t patrickhaahr/openhome-api:latest .

# Build the API image for amd64 + arm64 and push it to Docker Hub
[group('api')]
[working-directory: 'api']
api-docker-push version:
    docker buildx build --builder openhome-publisher --platform linux/amd64,linux/arm64 -t patrickhaahr/openhome-api:{{version}} -t patrickhaahr/openhome-api:latest --push .

# Format, typecheck, lint, and test the Expo client in one pass
[group('expo')]
[working-directory: 'mobile-expo']
mobile-check: expo-format expo-lint expo-check

# Compile both firmware targets in one pass
[group('firmware')]
firmware-check: ir-build switchbot-build

# Format, lint, and test the OpenHome CLI in one pass
[group('cli')]
[working-directory: 'cli']
cli-check: cli-format cli-lint cli-test

check-api: api-fmt api-lint api-test
check-mcp: mcp-check
check-mobile: mobile-check
check-firmware: firmware-check
check-cli: cli-check

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

# Apply supported Expo lint fixes
[group('expo')]
[working-directory: 'mobile-expo']
expo-lint-fix:
    bun run lint --fix

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
