default:
    @just --list

# Full gate across repos
check: check-api check-mcp check-mobile check-firmware check-cli check-dashboard deployment-check

# Remove API, MCP and CLI Rust build artifacts (next build recompiles)
[group('build')]
clean:
    rm -rf api/target mcp/target cli/target
    @echo "Rust targets wiped; rebuild will take a while."

# Show this help grouped by area
groups:
    @just --list --list-heading '' --justfile '{{justfile()}}'

# Read or administer the LG TV through the task's authenticated SSH connection
[group('tv')]
[positional-arguments]
tv-ssh *args:
    sh tv/tools/ssh.sh "$@"

# Reconnect the task's SSH control socket after a TV reboot (prompts for password)
[group('tv')]
tv-connect:
    sh tv/tools/connect.sh

# Save a private, local backup of the LG TV before or after changes
[group('tv')]
tv-snapshot label:
    python3 tv/tools/snapshot.py {{quote(label)}}

# Capture the TV's actual display into the private evidence directory
[group('tv')]
tv-screen label:
    python3 tv/tools/screen.py {{quote(label)}}

# Install the locally checked Pear package without a boot hook
[group('tv')]
tv-install-pear: tv-check
    python3 tv/tools/install-pear.py

# Enable the reviewed Alexa/privacy and Pear boot hooks after live validation
[group('tv')]
tv-install-hooks: tv-check
    python3 tv/tools/install-hooks.py

# Run a reviewed shell script on the TV with optional arguments
[group('tv')]
tv-script file args='':
    sh tv/tools/ssh.sh 'sh -s -- {{args}}' < {{quote(file)}}

# Validate TV scripts and build Pear against the locally backed-up LG Home files
[group('tv')]
tv-check: tv-assets
    sh -n tv/tools/ssh.sh
    sh -n tv/runtime/alexa-block.sh
    sh -n tv/tools/probe.sh
    sh -n tv/runtime/privacy-settings.sh
    sh -n tv/runtime/privacy-hosts.sh
    sh -n tv/runtime/boot.sh
    sh -n tv/tools/connect.sh
    sh -n tv/runtime/client-service.sh
    node --check tv/bridge/client.js
    node --check tv/bridge/preview.js
    node --check tv/tools/verify-client.js
    python3 -c 'import ast; from pathlib import Path; [ast.parse(p.read_text(), filename=str(p)) for directory in ("tv/runtime", "tv/tools") for p in Path(directory).glob("*.py")]'
    node tv/bridge/test.js
    python3 tv/tools/test-inputhook-startup.py
    qmlformat tv/launcher/OpenHome.qml tv/launcher/ui/*.qml > /dev/null
    python3 tv/tools/build-pear.py

# Render dashboard icons and agent vectors for LG's Qt PNG image loader
[group('tv')]
tv-assets:
    node tv/tools/export-icons.mjs

# Update OpenHome TV UI and its local client, preserving the original Pear UI
[group('tv')]
tv-deploy: tv-check
    python3 tv/tools/deploy-launcher.py

# Bind red to the native Cinema/night profile toggle without replacing other shortcuts
[group('tv')]
tv-picture-shortcut: tv-check
    python3 tv/tools/install-picture-shortcut.py

# Refresh Input Hook's elevated service registration before its normal boot startup
[group('tv')]
tv-inputhook-startup: tv-check
    python3 tv/tools/install-inputhook-startup.py

# Read-only acceptance checks from the TV's native Node runtime
[group('tv')]
tv-client-check:
    sh tv/tools/ssh.sh 'node -' < tv/tools/verify-client.js

# Save actual TV screenshots with synthetic health/weather, then restore live data
[group('tv')]
tv-preview-capture:
    python3 tv/tools/capture-preview.py

# Restore the exact setting values changed during TV setup
[group('tv')]
tv-restore-settings:
    python3 tv/tools/restore-settings.py

# Assert live Alexa blocking, privacy/preload settings, resolver and local control
[group('tv')]
tv-verify:
    python3 tv/tools/verify-live.py

# Add an API dependency and update its lockfile
[group('api')]
[working-directory: 'api']
api-add-package package *args:
    cargo add {{quote(package)}} {{args}}

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

# Refresh the MCP test harness lock after changing the local API dependency
[group('mcp')]
[working-directory: 'mcp']
mcp-refresh-api-lock:
    cargo update --offline --package openhome-api

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

# Build the MCP image for amd64 + arm64 (Raspberry Pi) locally
[group('mcp')]
mcp-docker-build version: docker-builder
    docker buildx build --builder openhome-local --platform linux/amd64,linux/arm64 -f mcp/Dockerfile --output type=oci,dest=.devenv/openhome-mcp.tar -t patrickhaahr/openhome-mcp:{{version}} -t patrickhaahr/openhome-mcp:latest .

# Build the MCP image for amd64 + arm64 and push it to Docker Hub
[group('mcp')]
mcp-docker-push version: docker-builder
    docker buildx build --builder openhome-local --platform linux/amd64,linux/arm64 -f mcp/Dockerfile -t patrickhaahr/openhome-mcp:{{version}} -t patrickhaahr/openhome-mcp:latest --push .

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

# Run one API unit test without rebuilding every integration-test binary
[group('api')]
[working-directory: 'api']
api-test-unit name:
    cargo test --lib {{name}} -- --exact

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

# Run the actual API against a separate private SQLite database for local verification
[group('api')]
[working-directory: 'api']
api-run-isolated database_url:
    SQLX_OFFLINE=true DATABASE_URL={{quote(database_url)}} cargo run

# Apply main API migrations to DATABASE_URL
[group('api')]
[working-directory: 'api']
api-migrate:
    sqlx migrate run

# Revert the latest main API migration
[group('api')]
[working-directory: 'api']
api-migrate-revert:
    sqlx migrate revert

# Regenerate the offline SQLx query cache (api/.sqlx) used by the SQLX_OFFLINE Docker build
[group('api')]
[working-directory: 'api']
api-sqlx-prepare:
    cargo sqlx prepare -- --all-targets

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

# Create a reversible Food database migration (just api-food-migration-add search_terms); the API applies them on start
[group('api')]
[working-directory: 'api']
api-food-migration-add name:
    sqlx migrate add -r --source food_migrations {{name}}

# Build the API image for amd64 + arm64 (Raspberry Pi) locally
[group('api')]
[working-directory: 'api']
api-docker-build version: docker-builder
    docker buildx build --builder openhome-local --platform linux/amd64,linux/arm64 --output type=oci,dest=../.devenv/openhome-api.tar -t patrickhaahr/openhome-api:{{version}} -t patrickhaahr/openhome-api:latest .

# Build the API image for amd64 + arm64 and push it to Docker Hub
[group('api')]
[working-directory: 'api']
api-docker-push version: docker-builder
    docker buildx build --builder openhome-local --platform linux/amd64,linux/arm64 -t patrickhaahr/openhome-api:{{version}} -t patrickhaahr/openhome-api:latest --push .

# Build the native Android app and its device tests
[group('mobile')]
[working-directory: 'mobile']
mobile-build:
    ./gradlew --console=plain :app:assembleDebug :e2e:assembleDebug

# Run Android Lint on the native app (warnings fail)
[group('mobile')]
[working-directory: 'mobile']
mobile-lint:
    ./gradlew --console=plain :app:lintDebug

# Create a persistent user-local release identity once (never replaces an existing key)
[group('mobile')]
mobile-release-key-init:
    bash mobile/tools/release.sh init

# Lint and build the release variant, then align, sign and verify without a device
[group('mobile')]
mobile-release:
    cd mobile && ./gradlew --console=plain :app:lintRelease :app:assembleRelease
    bash mobile/tools/release.sh sign
    just mobile-release-verify

# Show the APK certificate, release manifest and checksum without accessing a device
[group('mobile')]
mobile-release-verify:
    bash mobile/tools/release.sh verify

# Install the verified release on an explicit authorized device, preserving app data
[group('mobile')]
mobile-release-install serial: mobile-release-verify
    adb -s {{quote(serial)}} install -r mobile/app/build/outputs/apk/release/app-release.apk

# Run the native app's device tests on a booted emulator (optionally one class: just mobile-test SetupFlowTest)
[group('mobile')]
[working-directory: 'mobile']
mobile-test class='':
    #!/usr/bin/env bash
    set -euo pipefail
    # Emulators only: the tests clear the debug app's data, which must not reach a real phone's copy.
    serial="$(adb devices | awk '$1 ~ /^emulator-/ && $2 == "device" { print $1; exit }')"
    if [ -z "$serial" ]; then
        echo "mobile-test needs a booted Android emulator" >&2
        exit 1
    fi
    # Android allows one UiAutomation client; agent-device's snapshot helper holds it while a session is open.
    if [ -n "$(adb -s "$serial" shell pidof com.callstack.agentdevice.snapshothelper | tr -d '\r')" ]; then
        echo "mobile-test needs UiAutomation free: run 'agent-device close' first" >&2
        exit 1
    fi
    filter=()
    if [ -n {{quote(class)}} ]; then
        filter=("-Pandroid.testInstrumentationRunnerArguments.class=com.patrickhaahr.openhome.e2e."{{quote(class)}})
    fi
    # The emulator's host process grows with what it renders, past 10 GB over the whole suite, which can starve
    # the machine and the tests with it. So a run gives it room: built first, build daemons gone, no animations.
    ./gradlew --console=plain :app:assembleDebug :e2e:assembleDebug
    ./gradlew --console=plain --stop
    animations=(window_animation_scale transition_animation_scale animator_duration_scale)
    declare -A animation_was
    for scale in "${animations[@]}"; do
        animation_was[$scale]="$(adb -s "$serial" shell settings get global "$scale" | tr -d '\r')"
        adb -s "$serial" shell settings put global "$scale" 0
    done
    restore_animations() {
        for scale in "${animations[@]}"; do
            if [ "${animation_was[$scale]}" = null ]; then
                adb -s "$serial" shell settings delete global "$scale" >/dev/null || true
            else
                adb -s "$serial" shell settings put global "$scale" "${animation_was[$scale]}" || true
            fi
        done
    }
    trap restore_animations EXIT
    ANDROID_SERIAL="$serial" ./gradlew --console=plain :e2e:connectedDebugAndroidTest "${filter[@]}"

# Install the debug build on a booted emulator or device
[group('mobile')]
[working-directory: 'mobile']
mobile-install:
    ./gradlew --console=plain :app:installDebug

# Run the real API on :8000 with an invented key against a throwaway database, or restart it on an earlier one's directory (the emulator reaches it at 10.0.2.2:8000)
[group('mobile')]
mobile-fixture-api key='mobile-fixture-key' dir='':
    #!/usr/bin/env bash
    set -euo pipefail
    repo="$PWD"
    (cd api && SQLX_OFFLINE=true cargo build --bin openhome-api)
    # Outside the repo so the API's dotenv lookup finds no api/.env and stays on invented settings.
    dir={{quote(dir)}}
    if [ -z "$dir" ]; then dir="$(mktemp -d /tmp/openhome-mobile-fixture.XXXXXX)"; fi
    cd "$dir"
    echo "Throwaway database: $dir/app.db"
    DATABASE_URL="sqlite:$dir/app.db" API_KEY={{quote(key)}} NOOP_PUSH_TOKEN=mobile-fixture-noop-token \
        exec "$repo/api/target/debug/openhome-api"

# Fill an empty fixture API with invented fitness data larger than the API's list pages
[group('mobile')]
mobile-fixture-seed key='mobile-fixture-key':
    python3 mobile/tools/seed_fixture.py --api-key {{quote(key)}}

# Proxy the fixture API on :8001, committing the next N Workout creates, M Exercise creates, E Workout edits, B Body Weight records and P Profile edits but losing their responses (the emulator reaches it at 10.0.2.2:8001)
[group('mobile')]
mobile-fixture-lossy-proxy lose='1' lose-exercises='0' lose-edits='0' lose-body-weight='0' lose-profile='0':
    python3 mobile/tools/lossy_proxy.py --lose-workout-responses {{quote(lose)}} --lose-exercise-responses {{quote(lose-exercises)}} --lose-edit-responses {{quote(lose-edits)}} --lose-body-weight-responses {{quote(lose-body-weight)}} --lose-profile-responses {{quote(lose-profile)}}

# Lint and build the native app, then run its device tests when an emulator is booted
[group('mobile')]
mobile-check: mobile-lint mobile-build
    #!/usr/bin/env bash
    set -euo pipefail
    if adb devices | awk '$1 ~ /^emulator-/ && $2 == "device" { found = 1 } END { exit !found }'; then
        just mobile-test
    else
        echo "mobile-check: NO EMULATOR BOOTED, device tests NOT run. Boot one and run: just mobile-test" >&2
    fi

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
check-dashboard: dashboard-check

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

# Pull the published API, MCP and dashboard images, recreate changed containers, and wait for health
[group('deployment')]
openhome-deploy:
    python3 scripts/deploy.py activate

# Create/verify a local-only builder and enable ARM64 emulation for runtime image steps
[group('deployment')]
docker-builder:
    python3 scripts/deploy.py builder

# Patch-bump, check, publish and deploy only the areas changed since their last release (MCP follows API);
# name areas (api mcp dashboard) to force them. Backs up both databases and verifies the whole stack.
[group('deployment')]
deploy *areas:
    python3 scripts/deploy.py deploy {{areas}}

# Retry the current versions after fixing an interrupted release
[group('deployment')]
deploy-current:
    python3 scripts/deploy.py current

# Verify deployed versions, container health, API/MCP calls, HTTPS and dashboard assets
[group('deployment')]
openhome-verify:
    python3 scripts/deploy.py verify

# Check release automation without publishing images or restarting services
[group('deployment')]
deployment-check:
    python3 -m py_compile scripts/deploy.py scripts/deploy-pi.py
    python3 -m unittest discover -s scripts -p 'test_*.py'
    node --check scripts/deploy-smoke.mjs

# Dashboard recipes live in their own file to keep this one under the repository's file-size cap.
import 'just/dashboard.just'
