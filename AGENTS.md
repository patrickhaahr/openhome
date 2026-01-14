# AGENTS.md - Guidelines for AI Coding Agents

This is a homelab control app with Tauri v2 mobile frontend (SolidJS + TypeScript) and Rust axum backend API.

## Project Structure

```
home-app/
├── api/              # Rust axum backend (rpi-api)
├── mobile/           # Tauri v2 + SolidJS frontend
└── PLAN.md           # Full architecture spec
```

## Build & Test Commands

### API (Rust axum)
| Command | Description |
|---------|-------------|
| `cd api && cargo build` | Build project |
| `cd api && cargo build --release` | Release build |
| `cd api && cargo run` | Development mode |
| `cd api && cargo clippy` | Linter |
| `cd api && cargo clippy -- -D warnings` | Treat warnings as errors |
| `cd api && cargo fmt` | Format code |
| `cd api && cargo test` | Run all tests |
| `cd api && cargo test test_name -- --exact` | Run single test |

### Mobile (Tauri + SolidJS)
| Command | Description |
|---------|-------------|
| `cd mobile && bun start` | Start Vite dev server |
| `cd mobile && bun run dev` | Run Tauri Android dev build |
| `cd mobile && bun run build` | Build Tauri bundle + Vite frontend |
| `cd mobile && bun run serve` | Preview Vite production build |
| `cd mobile && bun run check` | Run Rust clippy (`cargo clippy -- -D warnings`) |

### Testing
- **Rust**: `cargo test` in api/ or mobile/src-tauri/ (use `--exact` for single test)
- **SolidJS**: No tests configured yet - use Vitest when adding (`bun vitest run <test-file>`)

## Code Style Guidelines

### Rust (API & Tauri)
- Run `cargo fmt` and `cargo clippy -- -D warnings` before committing
- Use `async fn`, `Result<T, E>`, `Option<T>`, and `?` for errors
- Avoid `.unwrap()`/`.expect()` except in tests/main
- Explicit error context: `.map_err(|e| Error::NetworkFailed(format!("Failed: {}", e)))?`

**Import Order:**
1. `use std::` → 2. `use tokio::` → 3. External crates → 4. Internal → 5. `use crate::`

**Naming:** PascalCase structs/enums, snake_case functions/modules, SCREAMING_SNAKE_CASE constants

**Error Handling:** Consistent JSON `{"error": "msg", "status": 401}` with status codes (401/404/422/502/503/504)

### TypeScript/SolidJS (Frontend)
- Use explicit types over implicit inference
- Define interfaces for all props and data structures
- Use `Component<Props>` type for SolidJS components

**Critical SolidJS Rule:** Components execute exactly once. Think "setup function" not "render function".
- Never destructure props - access as `props.name` not `{ name }`
- Use `createSignal` with getter/setter pattern - never destructure
- Use `createMemo` for expensive derived computations
- Use `<For>`, `<Show>`, `<Switch>`, `<Match>` for control flow (never `.map()` in JSX)

**Import Order:**
1. External packages (SolidJS, Tauri, etc.)
2. Internal modules (components, hooks, stores, utils)
3. Relative imports

**Naming:** PascalCase components, camelCase hooks with `use` prefix, singular noun stores

### Tauri-Specific
- Use capability-based permissions in `mobile/src-tauri/capabilities/`
- Define granular permissions per window
- Validate all command inputs, return meaningful error messages
- Use Tauri state for application-wide resources

## Security

- API: Constant-time API key comparison, validate inputs, env vars for secrets, no logging of secrets
- Tauri: Follow capability model - define minimal permissions
- Never expose sensitive data in error messages

## Key Resources

- Full architecture: `PLAN.md`
- API guidelines: `api/AGENTS.md` - Docker, IR, AdGuard routes, caching patterns
- Frontend guidelines: `mobile/AGENTS.md` - SolidJS patterns, Tauri commands, stores
- Use 'Axum' or 'Tauri' or 'SolidJS' skills for comprehensive detailed patterns
