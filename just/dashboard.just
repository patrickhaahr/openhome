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

# Fail when a stylesheet outside the allowlist defines a plain class selector (the Tailwind ratchet)
[group('dashboard')]
[working-directory: 'dashboard']
dashboard-check-css:
    bun scripts/check-css.ts

# Format, lint, typecheck, check CSS, test, and build the dashboard in one pass
[group('dashboard')]
[working-directory: 'dashboard']
dashboard-check: dashboard-format dashboard-lint dashboard-typecheck dashboard-check-css dashboard-test dashboard-build

# Smoke-test resized photos and serve approved albums on loopback; requires IMMICH_PUBLIC_DEMO=true
[group('dashboard')]
[working-directory: 'api']
dashboard-photos-preview:
    cargo run --locked --example photos_preview

# Compare the working tree's Dashboard with master's, scenario by scenario (optional scenario id filters)
[group('dashboard')]
[positional-arguments]
dashboard-parity *filters: dashboard-synthetic-build
    dashboard/tests/parity/parity.sh "$@"
