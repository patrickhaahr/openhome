default:
    @just --list

# Full gate across repos
check: check-api check-mcp check-mobile check-firmware check-cli check-dashboard deployment-check

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

expodir := 'mobile-expo'

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
check-dashboard: dashboard-check

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

# Build a standalone release APK without a connected device
[group('expo')]
[working-directory: 'mobile-expo']
expo-apk:
    bunx expo prebuild --platform android --no-install
    NODE_ENV=production android/gradlew -p android assembleRelease -Pandroid.buildToolsVersion=36.0.0 -Pandroid.compileSdkVersion=36 -Pandroid.targetSdkVersion=36

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

# Start the dashboard dev server on port 3000
[group('dashboard')]
[working-directory: 'dashboard']
dashboard-run:
    bunx vite dev --port 3000

# Inspect and capture dashboard browser evidence in a named agent-browser session
[group('dashboard')]
[positional-arguments]
dashboard-browser *args:
    agent-browser "$@"

# Inspect the attached iPad's exact Safari dashboard tab
[group('dashboard')]
[positional-arguments]
dashboard-ipad-inspect commands page_url="http://127.0.0.1:3000/" directory="http://127.0.0.1:9222/json":
    bun dashboard/.agents/skills/ipad-control/scripts/inspect.mjs "$1" "$2" --directory "$3"

# Build the dashboard
[group('dashboard')]
[working-directory: 'dashboard']
dashboard-build:
    bunx vite build

# Preview the dashboard production build
[group('dashboard')]
[working-directory: 'dashboard']
dashboard-preview:
    bunx vite preview

# Serve the dashboard production build with Bun
[group('dashboard')]
[working-directory: 'dashboard']
dashboard-start:
    bun .output/server/index.mjs

# Build the dashboard image for amd64 + arm64 (Raspberry Pi) locally
[group('dashboard')]
dashboard-docker-build version: docker-builder
    docker buildx build --builder openhome-local --platform linux/amd64,linux/arm64 -f dashboard/Dockerfile --output type=oci,dest=.devenv/openhome-dashboard.tar -t patrickhaahr/openhome-dashboard:{{version}} -t patrickhaahr/openhome-dashboard:latest dashboard

# Build the dashboard image for amd64 + arm64 and push it to Docker Hub
[group('dashboard')]
dashboard-docker-push version: docker-builder
    docker buildx build --builder openhome-local --platform linux/amd64,linux/arm64 -f dashboard/Dockerfile -t patrickhaahr/openhome-dashboard:{{version}} -t patrickhaahr/openhome-dashboard:latest --push dashboard

# Load the locally built Dashboard OCI images for container verification
[group('dashboard')]
dashboard-docker-load:
    docker load --input .devenv/openhome-dashboard.tar

# Verify a dashboard image's rendered page, static assets and private configuration
[group('dashboard')]
dashboard-docker-smoke image='patrickhaahr/openhome-dashboard:latest':
    #!/usr/bin/env bash
    set -euo pipefail
    container=$(docker run --rm -d -e API_KEY=synthetic-dashboard-smoke-key -e OPENHOME_API_URL=http://127.0.0.1:1 {{quote(image)}})
    trap 'docker rm -f "$container" >/dev/null' EXIT
    docker cp dashboard/tests/docker-smoke.ts "$container:/tmp/openhome-docker-smoke.ts"
    docker exec "$container" bun /tmp/openhome-docker-smoke.ts

# Stop the stack, pull the published API, MCP and dashboard images, and wait for healthy containers
[group('deployment')]
openhome-deploy:
    python3 scripts/deploy.py activate

# Run dashboard tests
[group('dashboard')]
[working-directory: 'dashboard']
dashboard-test *args: dashboard-health-fixture-build
    bunx vitest run {{args}}

# Build the isolated authenticated Axum health/list/fitness/map seam for dashboard tests
[group('dashboard')]
[working-directory: 'api']
dashboard-health-fixture-build:
    cargo build --locked --example dashboard_health

# Serve isolated authenticated health/lists/fitness/maps for synthetic dashboard previews
[group('dashboard')]
[working-directory: 'api']
dashboard-health-fixture-run: dashboard-health-fixture-build
    target/debug/examples/dashboard_health

# Build the synthetic API behind the dashboard preview
[group('dashboard')]
[working-directory: 'api']
dashboard-synthetic-build:
    cargo build --locked --example dashboard_preview

# Serve only the synthetic preview API on 127.0.0.1:3101 (override with OPENHOME_PREVIEW_API_PORT)
[group('dashboard')]
[working-directory: 'api']
dashboard-synthetic-api: dashboard-synthetic-build
    #!/usr/bin/env bash
    set -euo pipefail
    # Live TMDB search when a read token is available; the value is passed through, never printed.
    token_file="$HOME/.config/openhome/tmdb-read-access-token"
    if [ -z "${TMDB_READ_ACCESS_TOKEN:-}" ] && [ -r "$token_file" ]; then TMDB_READ_ACCESS_TOKEN="$(cat "$token_file")"; fi
    TMDB_READ_ACCESS_TOKEN="${TMDB_READ_ACCESS_TOKEN:-}" exec target/debug/examples/dashboard_preview

# Preview every dashboard page at http://localhost:3001/ on invented, deterministic data (Ctrl-C stops both;
# override the ports with OPENHOME_PREVIEW_API_PORT and OPENHOME_PREVIEW_DASHBOARD_PORT)
[group('dashboard')]
dashboard-synthetic: dashboard-synthetic-build
    #!/usr/bin/env bash
    set -euo pipefail
    port="${OPENHOME_PREVIEW_API_PORT:-3101}"
    dashboard_port="${OPENHOME_PREVIEW_DASHBOARD_PORT:-3001}"
    api_url="http://127.0.0.1:$port"
    api_key=synthetic-dashboard-api-key
    pids=()
    stop() {
        trap - EXIT INT TERM
        for pid in "${pids[@]}"; do pkill -TERM -P "$pid" 2>/dev/null || true; kill "$pid" 2>/dev/null || true; done
        wait 2>/dev/null || true
    }
    trap stop EXIT
    trap 'exit 130' INT TERM
    # Live TMDB search when a read token is available; the value is passed through, never printed.
    token_file="$HOME/.config/openhome/tmdb-read-access-token"
    if [ -z "${TMDB_READ_ACCESS_TOKEN:-}" ] && [ -r "$token_file" ]; then TMDB_READ_ACCESS_TOKEN="$(cat "$token_file")"; fi
    TMDB_READ_ACCESS_TOKEN="${TMDB_READ_ACCESS_TOKEN:-}" OPENHOME_PREVIEW_API_PORT="$port" api/target/debug/examples/dashboard_preview &
    pids+=($!)
    for _ in $(seq 1 100); do
        curl -fsS -H "Authorization: Bearer $api_key" "$api_url/api/health" >/dev/null 2>&1 && break
        kill -0 "${pids[0]}" 2>/dev/null || { echo "Synthetic API exited during startup" >&2; exit 1; }
        sleep 0.2
    done
    echo "Synthetic API ready at $api_url; dashboard at http://localhost:$dashboard_port/"
    # Explicit values override anything inherited, so the preview can never reach a real API.
    # IDLE_PREVIEW lets ?idle=now open the Idle Slideshow at once; production never sets it.
    (cd dashboard && OPENHOME_API_URL="$api_url" API_KEY="$api_key" IDLE_PREVIEW=true exec bunx vite dev --port "$dashboard_port" --strictPort) &
    pids+=($!)
    wait -n

# Watch dashboard tests
[group('dashboard')]
[working-directory: 'dashboard']
dashboard-test-watch: dashboard-health-fixture-build
    bunx vitest

# Lint the dashboard
[group('dashboard')]
[working-directory: 'dashboard']
dashboard-lint:
    bunx oxlint --type-aware --deny-warnings

# Apply supported dashboard lint fixes
[group('dashboard')]
[working-directory: 'dashboard']
dashboard-lint-fix:
    bunx oxlint --type-aware --fix

# Format the dashboard with oxfmt
[group('dashboard')]
[working-directory: 'dashboard']
dashboard-format:
    bunx oxfmt --write .

# Typecheck the dashboard
[group('dashboard')]
[working-directory: 'dashboard']
dashboard-typecheck:
    bunx tsc --noEmit

# Generate dashboard routes
[group('dashboard')]
[working-directory: 'dashboard']
dashboard-routes:
    bunx tsr generate

# Install dashboard dependencies from the lockfile
[group('dashboard')]
[working-directory: 'dashboard']
dashboard-install:
    bun install --frozen-lockfile

# Add a dashboard dependency and update its lockfile
[group('dashboard')]
[working-directory: 'dashboard']
dashboard-add-package package *args:
    bun add {{quote(package)}} {{args}}

# Run the dashboard's TanStack Intent CLI (just dashboard-intent list; just dashboard-intent load <package>#<skill>)
# Calls the package CLI directly: @tanstack/devtools also ships an `intent` bin that shadows it.
[group('dashboard')]
[working-directory: 'dashboard']
[positional-arguments]
dashboard-intent *args:
    node node_modules/@tanstack/intent/dist/cli.mjs "$@"

# Add a shadcn component to the dashboard
[group('dashboard')]
[working-directory: 'dashboard']
dashboard-add component:
    bunx --bun shadcn@latest add {{quote(component)}}

# Format, lint, typecheck, test, and build the dashboard in one pass
[group('dashboard')]
[working-directory: 'dashboard']
dashboard-check: dashboard-format dashboard-lint dashboard-typecheck dashboard-test dashboard-build

# Smoke-test resized photos and serve approved albums on loopback; requires IMMICH_PUBLIC_DEMO=true
[group('dashboard')]
[working-directory: 'api']
dashboard-photos-preview:
    cargo run --locked --example photos_preview

# Create/verify a local-only builder and enable ARM64 emulation for runtime image steps
[group('deployment')]
docker-builder:
    python3 scripts/deploy.py builder

# Patch-bump API/MCP/dashboard, check, publish both platforms, back up, deploy, and verify
[group('deployment')]
deploy:
    python3 scripts/deploy.py deploy

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
