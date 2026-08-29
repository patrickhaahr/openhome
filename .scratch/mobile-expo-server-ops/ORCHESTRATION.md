# Orchestration Plan — mobile-expo server-ops tickets 02–06

Orchestrator: Hermes (this session). Implementer: opencode via `/implement` command.
Working repo: /home/hermes/dev/openhome (branch master). Source of truth:
`.scratch/mobile-expo-server-ops/spec.md` + `.scratch/mobile-expo-server-ops/issues/`.

## Ticket order (fixed by Blocked-by chain; strictly sequential in main checkout)

- 02 container list + Docker Health Summary (blocked by 01: done)
- 03 lifecycle actions, 04 container logs (both blocked by 02)
- 05 compact timeline (blocked by 01), 06 feed manager (blocked by 05)

No worktrees: tickets 03/04 both mutate the Docker Tab created by 02, and 06 mutates
the timeline built by 05 — parallel worktrees would create guaranteed conflicts in
shared scaffolding (open-home-api.ts, docker/rss machines, Server/Docker screens).
Sequential with a green validation gate per ticket is faster end-to-end.

## Environment (repaired this session)

- flake.nix: android-cli hash was stale (Google rotated the "latest" artifact).
  Fixed to sha256-SD4puc2k9YeBGEhLmXJjiVJCqc3ayPnA05Gk71AkQNY= (verified by download).
- Dev shell: `nix develop -c <cmd>` from repo root. just/bun are NOT on host PATH.
- bun is not in the dev shell; wrap it in: `export PATH=/tmp/oh-bin:$PATH`
  (/tmp/oh-bin/bun -> /nix/store/dynr642xrg1fib1y2ry70hybjmzvqz3y-bun-1.3.13/bin/bun).
- mobile-expo/node_modules was empty → run `just expo-install` once before ticket 02.

## Validation matrix (run inside `nix develop -c bash -c '...PATH wrap...'`)

- Per ticket: `just expo-typecheck` + `just expo-test <feature-filter>` + `just expo-lint`.
- Final gate once after ticket 06: `just expo-check` (typecheck + tests + android export).
- No Rust changes → API tests/lint not required (spec: no backend work).

## Per-ticket loop

1. `opencode run --command implement '<prompt>' -f <spec> -f <ticket> -f <CONTEXT.md>`
   in repo root, background, monitor via process poll. Prompt pins: scope (this
   ticket only, mobile-expo/ only), validation commands, "do NOT git commit",
   finish by running the validation matrix and reporting results.
2. /implement runs reviewer subagent (fix loop, max 3 calls) then simplifier once —
   per the command definition; leave that to opencode.
3. Hermes reviews the diff against the ticket checkboxes + spec decisions
   (adapter guards, PascalCase/snake_case tolerance, 15s docker timeout, no confirm
   dialogs, 200-line tail, before_id pagination, delete+undo batch re-add).
4. If clean: `opencode run 'Use the commit skill...'` → ONE-line conventional commit.
   If findings: targeted follow-up `opencode run -c <fixes>` then re-review.
5. Hermes flips the ticket status to done in .scratch (with commit SHAs) and commits
   `docs: mark server-ops ticket 0N done` (one line, matches history).

## Conventions from ticket 01 (follow these)

- Application machines: src/application/use-<feature>.ts + colocated .test.ts using
  scripted fake adapter. Domain: src/domain/<feature>.ts (+ json guards in domain/json.ts).
- Adapter: src/infrastructure/open-home-api.ts namespaced method groups, Result returns.
- UI: render-only components in src/ui/, shared primitives in src/ui/shared.tsx.
- Commit style: `feat(mobile-expo): <one line>` + short body (01 had a body; ticket
  commits here are one line per user instruction).
- Global agent rules (~/.agents/AGENTS.md): surgical changes, simplicity first,
  type-driven design, meaningful tests over shallow coverage.

## Escalation

- opencode run failure/model errors → retry once; then pin nothing (auth is provider
  "OpenCode Go") and if still failing, report to user on Signal.
- Validation failure opencode can't fix in its own loop → Hermes debugs the diff
  directly before committing.
- Never push; never touch api/ or mobile/ (deprecated Tauri client stays frozen).

## Completion

- All 5 tickets committed + marked done; final gate green; `git status` clean;
  Signal ping to Patrick (`hermes send --to signal:Patrick`) with summary.
