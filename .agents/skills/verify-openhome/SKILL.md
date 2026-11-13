---
name: verify-openhome
description: Launch an isolated synthetic OpenHome stack (Axum API + Dashboard + Hermes MCP adapter) for this checkout, drive features the way the household and Hermes do (MCP tool calls, authenticated API reads, the Dashboard in a T3 Code preview tab), and capture proof. Use when proving an API, MCP or Dashboard change works beyond unit tests, or when a ticket asks for observed behaviour.
---

# Verify OpenHome

Proves behaviour on a real stack: production Axum routes, migrations and authentication over in-memory databases seeded with invented data, loopback stand-ins for every upstream (TMDB unless a token is present, Immich, openrouteservice, Open-Meteo, IR and SwitchBot bridges), the Dashboard dev server, and the real MCP adapter on top. It never touches the household API, credentials or the Pi.

All helpers live in `scripts/` next to this file. Run them from anywhere inside the checkout. Each checkout (worktree) gets its own run on free ports, so parallel worktrees never share an instance.

## Launch

```bash
.agents/skills/verify-openhome/scripts/up.sh
```

It builds the synthetic preview API (`just dashboard-synthetic-build`) and the MCP adapter, installs Dashboard dependencies if missing, then starts `just dashboard-synthetic` and the MCP adapter, each in its own process group. Ready means `doctor.sh` passes, which needs every surface to answer, not just a process. It prints `run.env`:

- `API_URL`, `DASHBOARD_URL`, `MCP_URL`, `API_KEY` (always `synthetic-dashboard-api-key`)
- `RUN_DIR`, `EVIDENCE_DIR`, the process group IDs

The first build in a fresh worktree takes several minutes (each worktree has its own `target/`). A warm start takes about 15 seconds. Logs: `$RUN_DIR/logs/{build,stack,mcp}.log`.

Load the run's values into a shell with `. "$(readlink -f ~/.local/state/openhome-verify/current-*)/run.env"` only when one run exists. With several worktrees running, use `OPENHOME_VERIFY_RUN=<run dir>` or read `run.env` from the path `up.sh` printed.

The seed data is invented and lives in the preview example's `seed/` directory: Household Users Alex, Robin and Sam, plus their lists, workouts, foods and so on. Data is in memory and resets on every launch.

## Doctor

```bash
.agents/skills/verify-openhome/scripts/doctor.sh
```

Read-only. It checks that both process groups are alive, the API answers with the key and refuses without one (so it is the synthetic preview, not something else on the port), the Dashboard serves, and MCP `tools/list` works. It prints the checkout's HEAD. Run it first whenever anything looks off. Never drive a run that `up.sh` in this checkout didn't start.

## Drive

**Hermes (MCP).** `scripts/mcp.sh` speaks Streamable HTTP like Hermes does: initialize, session, then one request.

```bash
S=.agents/skills/verify-openhome/scripts
$S/mcp.sh tools/list
$S/mcp.sh call household_users '{}'
$S/mcp.sh call add_watchlist_entry '{"owner_id":"<id>","kind":"movie","title":"Verify Harbour Lights","request_id":"verify-<run id>"}'
```

It prints the JSON-RPC response and exits 2 when the tool result has `isError: true`. Tool JSON is in `result.structuredContent` (and as text in `result.content[0].text`).

**Axum.** Use plain curl with the bearer key for read-backs: `curl -fsS -H "Authorization: Bearer $API_KEY" "$API_URL/api/..."`.

**Dashboard.** Use T3 Code's preview tools, not another browser:

1. `preview_open` with `target: {kind: "environment-port", port: <DASHBOARD_URL port>, path: "/lists?tab=watch"}`, `reuseExistingTab: false`, `open: false`. Keep the returned `tabId` and pass it to every later call. Parallel subagents each open their own tab.
2. `preview_wait_for` with the `text` you expect before reading.
3. `preview_snapshot` gives the accessibility tree. Act with `preview_act` using `locator: "aria-ref=<ref>"` from the latest snapshot. Refs die on navigation or a new snapshot.
4. Prefer accessible names: navigation `Main navigation` links (`Lists`, `Food`, ...), the `Lists` tab links `Wish` / `Watch` / `Dates`, the `Whose list` group of owner buttons, and buttons such as `Add a title`.
5. For the household iPad layout, use `preview_resize` with `mode: "preset", preset: "ipad-air"` and an `orientation`.
6. `t3_preview_close` your tab when done.

The feature map in `features/` holds the per-feature recipes. Read `features/README.md` first.

## Evidence

Put proof under `$EVIDENCE_DIR/<feature>/` with numbered names (`01-…json`, `05-dashboard.png`). Copy preview screenshots there from the `screenshotPath` that `preview_snapshot` with `save: true` returns. Evidence lives under `~/.local/state/openhome-verify/runs/<run id>/`, outside the checkout, so it survives worktree removal. Cite that path in PRs.

Proof standards:

- Drive the real user path: MCP tools as Hermes calls them, or Dashboard controls. Never call internal functions or seed rows directly to make a journey pass.
- Capture the action and the resulting state: the tool response, then an independent read-back (an API GET or the Dashboard showing it). For retries, capture both responses and the `replayed` flag.
- Check side effects, not only what's visible: a write must show up in a second view.
- Loopback upstream stand-ins are the only mocks allowed. They replace systems that the production code already isolates behind adapters. A feature whose upstream has no stand-in yet must add one to the preview example (see `features/README.md`), not mock inside the app.
- Report the proof level reached (built, activated, observed) and name every entry point not driven.

## Cleanup

```bash
.agents/skills/verify-openhome/scripts/down.sh
```

Stops only the two process groups this checkout's run started and removes the checkout's `current-*` link. Evidence stays at the printed path. Close any preview tab you opened. Never kill by process name: other worktrees may have runs up. Run `down.sh` after every failed attempt too, so you don't strand ports.

Delete old runs by hand when they're no longer needed: `~/.local/state/openhome-verify/runs/`.

## Helpers

| Script | Use |
|---|---|
| `scripts/up.sh` | Build and launch the stack, wait until usable, print `run.env` |
| `scripts/doctor.sh` | Read-only health of the current run |
| `scripts/mcp.sh tools/list` / `scripts/mcp.sh call <tool> '<json>'` | Call the MCP adapter like Hermes |
| `scripts/down.sh` | Stop this checkout's run, keep evidence |

Keep the feature map honest with `/pstack:maintain-verification-skill` when features change.
