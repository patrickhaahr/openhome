# AGENTS.md - Hermes MCP adapter

A Streamable HTTP MCP server that gives Hermes read-only training tools. Each request with representable path arguments makes exactly one authenticated GET against the OpenHome API and returns the API's JSON as the tool's structured content, unchanged. See root `AGENTS.md` for repo-wide guidelines.

## Boundaries (ADR 0002)

- The API owns interpretation. The adapter must never:
  - open either SQLite database,
  - compute dates or units,
  - pick NOOP sources,
  - judge freshness or coverage,
  - reshape a result.

  A new tool needs a new API endpoint first.
- Tools are read-only (`read_only_hint = true`, `open_world_hint = false`). Tool arguments become percent-encoded path segments. Literal `.` and `..` segments are rejected before sending a request because URL libraries normalize them. API redirects are never followed, so a call cannot move to another API path.
- Output caps are enforced by the API. It rejects oversized results with an error and never truncates them. The adapter returns that error as is.
- The adapter uses the existing `API_KEY`. There is no new credential model and no tailnet routing code.

## Tools

The tool contracts are documented in `docs/training-context.md`.

- `training_context(from_day, to_day)` → `GET /api/training/context/{from_day}/{to_day}`
  - 1–90 inclusive Copenhagen days, one entry per day. Summaries of logged Workouts per Exercise entry, the separate NOOP Workouts, daily Body Weight, sleep duration, resting HR and HRV, each day with recovery and workout freshness and coverage. Exact Sets and sleep sessions stay in the focused tools.
- `metric_trend(metric, from_day, to_day)` → `GET /api/training/trends/{metric}/{from_day}/{to_day}`
  - Exactly four metrics: daily Body Weight, sleep duration, resting heart rate, and HRV, over 1–90 inclusive Copenhagen days. The API returns daily values and ISO-week summaries with observation counts; the adapter passes them through.

- `recovery_on_day(day)` → `GET /api/training/days/{day}/recovery`
  - `day` is a Europe/Copenhagen calendar date, `YYYY-MM-DD`.
  - Returns the Recovery Day: sleep, resting HR, HRV, NOOP's derived scores, and `noop` freshness and coverage.
- `sleep_recent(n)` → `GET /api/training/sleep/recent/{n}`
  - `n` is 1–14 consecutive Copenhagen wake days, newest first. Missing nights remain explicit entries with `noop` freshness and coverage.
- `workouts_on_day(day)` → `GET /api/training/days/{day}/workouts`
  - `day` is a Europe/Copenhagen calendar date, `YYYY-MM-DD`.
  - Returns the Workouts logged in OpenHome that day with their Exercises and Sets, the separate NOOP Workouts starting that day, and `noop` freshness and coverage.
- `exercise_history(exercise_name, from_day, to_day)` → `GET /api/training/exercises/{exercise_name}/history/{from_day}/{to_day}`
  - Dates are inclusive Europe/Copenhagen calendar days, up to 90 days. Returns dated Workouts and exact ordered Sets for one resolved Exercise; an Exercise without performances has an empty `workouts` list.

Errors:
- Literal `.` or `..` arguments become a 400 tool error before any HTTP request. This is a URL transport constraint; date validation stays in the API.
- A non-2xx API answer with a JSON `{"error", "status"}` body becomes a tool error (`isError: true`) carrying that body. For example, 400 for an invalid day, or 422 when a session, Set or workout cap is exceeded.
- An unreachable API becomes `{"error": "The OpenHome API could not be reached", "status": null}`.
- A non-JSON answer becomes `{"error": "The OpenHome API answered <status> without a JSON …", "status": <code>}`.
- API requests time out after 15 s.

## Runtime settings

| Variable | Required | Meaning |
| --- | --- | --- |
| `OPENHOME_API_URL` | yes | Base URL of the OpenHome API, e.g. `http://openhome-api:8000`. Must be http(s). |
| `API_KEY` | yes | The API's `API_KEY`. MCP clients must send it as `Authorization: Bearer <API_KEY>`, and the adapter uses the same value for its own API calls. Leading or trailing whitespace is rejected. |
| `MCP_BIND_ADDR` | no | Listen address. Default `0.0.0.0:8001`. |
| `MCP_ALLOWED_HOSTS` | no | Comma-separated `Host` values the endpoint answers to (DNS-rebinding protection). Each entry is a host, which matches any port, or a `host:port`. Default is loopback only. Set it to the tailnet hostname(s) Hermes uses, e.g. `openhome,openhome.tailnet-name.ts.net`. A set but empty value is refused at startup, because rmcp would read an empty list as "allow every host". |
| `RUST_LOG` | no | Tracing filter. Default `info`. |

- **Endpoint**: `http://<host>:8001/mcp` (Streamable HTTP).
- **Authentication**: every request without the bearer key gets 401 with `WWW-Authenticate: Bearer`.
- **Shutdown**: SIGTERM or Ctrl+C closes open MCP sessions and then exits.
- **Deployment**: the image and Compose service are tracked in #45.

## Commands

Run everything from the repo root inside devenv:

- `just mcp-build`
- `just mcp-run`. Export `OPENHOME_API_URL` and `API_KEY` first. The adapter does not read `.env`.
- `just mcp-test`: the contract test. It runs the real API training routes in-process and drives the adapter with an rmcp client, covering discovery, a successful call, authentication failure and API error propagation.
- `just mcp-format`
- `just mcp-lint`

## Gotchas

- `serde_json` uses `float_roundtrip`, so API numbers are re-serialized exactly. Without it, values drift by one ulp between the API and the tool result.
- Tool descriptions come from `#[tool(description = CONST)]` single-line strings. rmcp joins doc-comment lines with `\n`, and clients would show those line breaks.
- Server info is set explicitly from `CARGO_PKG_NAME` and `CARGO_PKG_VERSION`. `Implementation::from_build_env()` would report rmcp's own name.
