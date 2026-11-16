#!/usr/bin/env bash
# Serves master's and the working tree's Dashboard as production builds against one synthetic API,
# then captures and diffs every scenario. Usage: parity.sh [scenario-id-substring ...]
set -euo pipefail
repo="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
cache="${XDG_CACHE_HOME:-$HOME/.cache}/openhome-parity"
master_sha="$(git -C "$repo" merge-base HEAD origin/master)"
checkout="$(printf %s "$repo" | sha256sum | cut -c1-12)"
master="$cache/master-$master_sha"
branch="$cache/branch-$checkout"
run="$(date +%Y%m%d-%H%M%S)"
run_dir="$repo/dashboard/.parity/$run"
chromium="$(command -v "${PARITY_CHROMIUM:-chromium}")" || { echo "No Chromium; set PARITY_CHROMIUM" >&2; exit 1; }
api_key=synthetic-dashboard-api-key
mkdir -p "$cache" "$run_dir/logs"

exec 9>"$branch.lock"
flock -n 9 || { echo "Another parity run is using this checkout" >&2; exit 1; }

# The production image builds from exactly these paths (dashboard/.dockerignore).
sources=(package.json bun.lock vite.config.ts tsconfig.json src public)

build() {
  (cd "$1" && bun install --frozen-lockfile && bun --bun x vite build) >"$run_dir/logs/build-$2.log" 2>&1 ||
    { echo "Building the $2 side failed; see $run_dir/logs/build-$2.log" >&2; return 1; }
}

echo "Building master ($master_sha) and the working tree..."
(
  flock 8
  if [ ! -f "$master/.built" ]; then
    rm -rf "$master"
    mkdir -p "$master"
    git -C "$repo" archive "$master_sha" "${sources[@]/#/dashboard/}" | tar -x -C "$master" --strip-components=1
    build "$master" master
    touch "$master/.built"
  fi
) 8>"$master.lock"
mkdir -p "$branch"
rsync -a --delete "${sources[@]/#/$repo/dashboard/}" "$branch/"
build "$branch" branch

free_port() { python3 -c 'import socket;s=socket.socket();s.bind(("127.0.0.1",0));print(s.getsockname()[1]);s.close()'; }
pgids=()
stop() {
  trap - EXIT INT TERM
  for pgid in "${pgids[@]}"; do kill -TERM -- "-$pgid" 2>/dev/null || true; done
  wait 2>/dev/null || true
}
trap stop EXIT
trap 'exit 130' INT TERM

# An empty token keeps the preview on its loopback TMDB catalogue, never the live service.
api_port="$(free_port)"
OPENHOME_PREVIEW_API_PORT="$api_port" TMDB_READ_ACCESS_TOKEN='' \
  setsid "$repo/api/target/debug/examples/dashboard_preview" >"$run_dir/logs/api.log" 2>&1 </dev/null &
pgids+=($!)
api_url="http://127.0.0.1:$api_port"

serve() {
  (cd "$1" && PORT="$3" HOST=127.0.0.1 OPENHOME_API_URL="$api_url" API_KEY="$api_key" IDLE_PREVIEW=true \
    exec setsid bun .output/server/index.mjs) >"$run_dir/logs/$2.log" 2>&1 </dev/null &
  pgids+=($!)
}

# The preview prints its URL only after seeding the household through its own routes.
for _ in $(seq 1 300); do
  grep -q "^http://" "$run_dir/logs/api.log" && break
  kill -0 "${pgids[0]}" 2>/dev/null || { echo "Synthetic API exited; see $run_dir/logs/api.log" >&2; exit 1; }
  sleep 0.2
done
before_port="$(free_port)"
after_port="$(free_port)"
serve "$master" master "$before_port"
serve "$branch" branch "$after_port"
before_url="http://127.0.0.1:$before_port"
after_url="http://127.0.0.1:$after_port"
for url in "$before_url" "$after_url"; do
  for _ in $(seq 1 100); do curl -fsS -o /dev/null "$url/connection" 2>/dev/null && break; sleep 0.2; done
  curl -fsS -o /dev/null "$url/connection" || { echo "Dashboard at $url never answered; see $run_dir/logs" >&2; exit 1; }
done

branch_sha="$(git -C "$repo" rev-parse --short HEAD)"
[ -z "$(git -C "$repo" status --porcelain -- dashboard)" ] || branch_sha="$branch_sha + uncommitted changes"
# Keep the five newest runs; find lists real directories only, never the `latest` link.
find "$repo/dashboard/.parity" -mindepth 1 -maxdepth 1 -type d | sort | head -n -5 | xargs -r rm -rf
ln -sfn "$run" "$repo/dashboard/.parity/latest"

cd "$repo/dashboard"
PARITY_API_URL="$api_url" PARITY_API_KEY="$api_key" PARITY_BEFORE_URL="$before_url" PARITY_AFTER_URL="$after_url" \
  PARITY_RUN="$run" PARITY_RUN_DIR="$run_dir" PARITY_CHROMIUM="$chromium" PARITY_FILTER="$*" \
  PARITY_MASTER="$(git -C "$repo" rev-parse --short "$master_sha")" PARITY_BRANCH="$branch_sha" \
  bun tests/parity/run.ts
