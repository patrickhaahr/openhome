default:
    @just --list

# Full gate across repos
check: check-api check-mcp check-mobile check-firmware check-cli check-dashboard deployment-check

# List files over the pre-commit hook's 5000-token cap, optionally under the given paths
[positional-arguments]
file-size-check *paths:
    @bash scripts/file-size-check.sh "$@"

# Remove API, MCP and CLI Rust build artifacts (next build recompiles)
[group('build')]
clean:
    rm -rf api/target mcp/target cli/target
    @echo "Rust targets wiped; rebuild will take a while."

# Show this help grouped by area
groups:
    @just --list --list-heading '' --justfile '{{justfile()}}'

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

# Alias for ir-upload
ir-flash: ir-upload

# Upload the IR remote firmware, then open its serial monitor
ir-run: ir-upload ir-monitor

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

# Area recipes live in their own files to keep each one under the repository's file-size cap.
import 'just/tv.just'
import 'just/api.just'
import 'just/mobile.just'
import 'just/firmware.just'
import 'just/dashboard.just'
