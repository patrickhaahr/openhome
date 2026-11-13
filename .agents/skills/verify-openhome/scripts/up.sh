#!/usr/bin/env bash
# Start an isolated synthetic OpenHome stack (Axum preview API + Dashboard + MCP adapter) for this checkout.
# Usage: up.sh            prints the run directory and URLs once every surface answers.
. "$(dirname "$0")/common.sh"
if [ -n "$RUN_DIR" ] && [ -f "$RUN_DIR/run.env" ]; then
  . "$RUN_DIR/run.env"
  if kill -0 "$STACK_PGID" 2>/dev/null; then echo "Run already up: $RUN_DIR (run down.sh first)" >&2; exit 1; fi
fi
RUN_ID="$(date +%Y%m%dT%H%M%S)-$$"
RUN_DIR="$STATE_ROOT/runs/$RUN_ID"
mkdir -p "$RUN_DIR/evidence" "$RUN_DIR/logs"
API_PORT="${OPENHOME_PREVIEW_API_PORT:-$(free_port)}"
DASHBOARD_PORT="${OPENHOME_PREVIEW_DASHBOARD_PORT:-$(free_port)}"
MCP_PORT="${OPENHOME_VERIFY_MCP_PORT:-$(free_port)}"
API_KEY=synthetic-dashboard-api-key
cd "$REPO_ROOT"
echo "Building the synthetic API and MCP adapter (first build in a fresh worktree takes minutes)..."
devenv shell -- just dashboard-synthetic-build mcp-build >"$RUN_DIR/logs/build.log" 2>&1 || { echo "Build failed; see $RUN_DIR/logs/build.log" >&2; exit 1; }
[ -d dashboard/node_modules ] || devenv shell -- just dashboard-install >>"$RUN_DIR/logs/build.log" 2>&1
# setsid gives the stack its own process group, so down.sh stops exactly what this run started.
OPENHOME_PREVIEW_API_PORT="$API_PORT" OPENHOME_PREVIEW_DASHBOARD_PORT="$DASHBOARD_PORT" \
  setsid devenv shell -- just dashboard-synthetic >"$RUN_DIR/logs/stack.log" 2>&1 < /dev/null &
STACK_PGID=$!
OPENHOME_API_URL="http://127.0.0.1:$API_PORT" API_KEY="$API_KEY" MCP_BIND_ADDR="127.0.0.1:$MCP_PORT" \
  setsid "$REPO_ROOT/mcp/target/debug/openhome-mcp" >"$RUN_DIR/logs/mcp.log" 2>&1 < /dev/null &
MCP_PGID=$!
cat > "$RUN_DIR/run.env" <<ENV
RUN_ID=$RUN_ID
RUN_DIR=$RUN_DIR
EVIDENCE_DIR=$RUN_DIR/evidence
REPO_ROOT=$REPO_ROOT
API_URL=http://127.0.0.1:$API_PORT
DASHBOARD_URL=http://localhost:$DASHBOARD_PORT
MCP_URL=http://127.0.0.1:$MCP_PORT/mcp
API_KEY=$API_KEY
STACK_PGID=$STACK_PGID
MCP_PGID=$MCP_PGID
ENV
ln -sfn "$RUN_DIR" "$CURRENT_LINK"
for _ in $(seq 1 600); do
  if "$(dirname "$0")/doctor.sh" >/dev/null 2>&1; then
    echo "Ready. Run dir: $RUN_DIR"; cat "$RUN_DIR/run.env"; exit 0
  fi
  kill -0 "$STACK_PGID" 2>/dev/null || { echo "Stack exited; see $RUN_DIR/logs/stack.log" >&2; exit 1; }
  sleep 1
done
echo "Stack not ready after 10 minutes; see $RUN_DIR/logs" >&2; exit 1
