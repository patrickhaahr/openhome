# MCP adapter transport rules

The adapter forwards each tool call as one authenticated API request and owns
no domain rules (ADRs 0002 and 0005). A few transport rules are its own:

- **One argument, one path segment.** Each segment is percent-encoded. Literal
  `.` and `..` segments are rejected before the URL is built, because URL
  construction would silently drop them and change the endpoint. Query
  punctuation in an owner ID stays inside one encoded value. Calendar
  validation stays with the API.
- **Bodies.** DELETE is the only API operation whose success has no JSON body,
  so a 204 becomes `{"deleted": true}`. An API error body (`{"error",
  "status"}`) passes through unchanged as the tool error.
- **Request size.** The request body limit is 16 MiB: a 10 MB meal photo is
  about 13.4 MiB once base64-encoded, plus the JSON-RPC envelope. rmcp's
  default is 4 MiB.
- **Hosts.** `MCP_ALLOWED_HOSTS` (comma-separated hosts or `host:port`,
  default loopback only) must not be empty. rmcp reads an empty list as "allow
  every host", which turns off DNS-rebinding protection.
- **Configuration.** Required: `OPENHOME_API_URL` and `API_KEY`. The API key
  authorises both MCP clients and the adapter's own API calls, and is never
  printed. `MCP_BIND_ADDR` defaults to `0.0.0.0:8001`.
- **Shutdown.** Shutdown cancels open MCP sessions, which would otherwise hold
  graceful shutdown open.
