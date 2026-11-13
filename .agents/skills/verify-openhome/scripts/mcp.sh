#!/usr/bin/env bash
# Call the run's MCP adapter over Streamable HTTP, the way Hermes does.
# Usage: mcp.sh tools/list
#        mcp.sh call <tool> '<json arguments>'
# Prints the JSON-RPC result; exits 2 when the tool result has isError=true.
. "$(dirname "$0")/common.sh"
load_run
H=(-H "Authorization: Bearer $API_KEY" -H "Content-Type: application/json" -H "Accept: application/json, text/event-stream")
body() { sed -n 's/^data: //p' | grep -v '^$' | tail -1; }
hdr="$(mktemp)"; trap 'rm -f "$hdr"' EXIT
curl -fsS -m 30 -D "$hdr" "${H[@]}" "$MCP_URL" \
  -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"verify-openhome","version":"1"}}}' >/dev/null
SID="$(grep -i '^mcp-session-id:' "$hdr" | cut -d' ' -f2 | tr -d '\r')"
S=(-H "Mcp-Session-Id: $SID")
curl -fsS -m 10 "${H[@]}" "${S[@]}" "$MCP_URL" -d '{"jsonrpc":"2.0","method":"notifications/initialized"}' >/dev/null
case "${1:-}" in
  tools/list) req='{"jsonrpc":"2.0","id":2,"method":"tools/list"}' ;;
  call) req="$(python3 -c 'import json,sys;print(json.dumps({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":sys.argv[1],"arguments":json.loads(sys.argv[2] if len(sys.argv)>2 else "{}")}}))' "$2" "${3:-{\}}")" ;;
  *) echo "usage: mcp.sh tools/list | mcp.sh call <tool> '<json>'" >&2; exit 64 ;;
esac
out="$(curl -fsS -m 120 "${H[@]}" "${S[@]}" "$MCP_URL" -d "$req" | body)"
echo "$out"
echo "$out" | python3 -c 'import json,sys;r=json.load(sys.stdin).get("result",{});sys.exit(2 if r.get("isError") else 0)'
