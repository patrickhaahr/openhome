#!/usr/bin/env bash
# Stop only the process groups this checkout's verify run started,. Evidence stays.
. "$(dirname "$0")/common.sh"
load_run
for pgid in "$MCP_PGID" "$STACK_PGID"; do
  kill -TERM -- "-$pgid" 2>/dev/null || true
done
for _ in $(seq 1 20); do
  kill -0 -- "-$STACK_PGID" 2>/dev/null || kill -0 -- "-$MCP_PGID" 2>/dev/null || break
  sleep 0.5
done
kill -KILL -- "-$STACK_PGID" "-$MCP_PGID" 2>/dev/null || true
rm -f "$CURRENT_LINK"
echo "Stopped run $RUN_ID. Evidence kept at $EVIDENCE_DIR"
