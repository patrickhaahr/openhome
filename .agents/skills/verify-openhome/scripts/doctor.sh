#!/usr/bin/env bash
# Read-only: is the current verify run worth driving? Exits non-zero naming the first failing surface.
. "$(dirname "$0")/common.sh"
load_run
fail() { echo "FAIL: $*"; exit 1; }
kill -0 "$STACK_PGID" 2>/dev/null || fail "stack process group $STACK_PGID is gone"
kill -0 "$MCP_PGID" 2>/dev/null || fail "MCP process group $MCP_PGID is gone"
curl -fsS -m 5 -H "Authorization: Bearer $API_KEY" "$API_URL/api/health" >/dev/null || fail "API $API_URL/api/health"
code=$(curl -s -o /dev/null -w '%{http_code}' -m 5 "$API_URL/api/health"); [ "$code" = 401 ] || fail "API answered $code without a key; is it the synthetic preview?"
curl -fsS -m 20 -o /dev/null "$DASHBOARD_URL/" || fail "Dashboard $DASHBOARD_URL"
"$(dirname "$0")/mcp.sh" tools/list >/dev/null || fail "MCP $MCP_URL tools/list"
echo "OK run=$RUN_ID api=$API_URL dashboard=$DASHBOARD_URL mcp=$MCP_URL head=$(git -C "$REPO_ROOT" rev-parse --short HEAD)"
