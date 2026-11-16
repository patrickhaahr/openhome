# Add an API dependency and update its lockfile
[group('api')]
[working-directory: 'api']
api-add-package package *args:
    cargo add {{quote(package)}} {{args}}

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
