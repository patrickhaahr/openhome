# AGENTS.md - Repository Agent Guide

Homelab control app with a Rust axum API and a Tauri v2 mobile frontend (SolidJS + TypeScript).
This document is the repo-wide guide; see `api/AGENTS.md` and `mobile/AGENTS.md` for deeper, service-specific rules.

## Project Structure

```
home-app/
├── api/              # Rust axum backend (rpi-api)
├── mobile/           # Tauri v2 + SolidJS frontend
└── PLAN.md           # Full architecture spec
```

## Build, Lint, and Test Commands

### API (Rust axum)

Run from repo root unless noted.

| Command | Description |
|---------|-------------|
| `cd api && cargo build` | Build API |
| `cd api && cargo build --release` | Release build |
| `cd api && cargo run` | Dev run (loads `.env` via dotenvy) |
| `cd api && cargo fmt` | Format code |
| `cd api && cargo clippy` | Lint (fix warnings) |
| `cd api && cargo clippy -- -D warnings` | Lint with warnings as errors |
| `cd api && cargo test` | Run all tests |
| `cd api && cargo test test_name -- --exact` | Run a single unit test |
| `cd api && cargo test module::test_name -- --exact` | Run a namespaced unit test |
| `cd api && cargo test --test integration_name` | Run one integration test file |

### Mobile (Tauri + SolidJS)

| Command | Description |
|---------|-------------|
| `cd mobile && bun start` | Vite dev server (frontend only) |
| `cd mobile && bun run dev` | Alias for `bun start` |
| `cd mobile && bun run android` | Tauri Android dev build |
| `cd mobile && bun run build` | Vite production build |
| `cd mobile && bun run serve` | Preview Vite build |
| `cd mobile && bun run check` | Rust clippy for `src-tauri/` (`-D warnings`) |
| `cd mobile && cd src-tauri && cargo fmt` | Format Tauri Rust code |
| `cd mobile && cd src-tauri && cargo test` | Run all Tauri Rust tests |
| `cd mobile && cd src-tauri && cargo test test_name -- --exact` | Run a single Tauri Rust test |

### Single Test Examples

- API unit test: `cd api && cargo test health_ok -- --exact`
- API namespaced test: `cd api && cargo test services::feed::tests::refresh_feed -- --exact`
- API integration test: `cd api && cargo test --test api_health`
- Mobile frontend test (when added): `cd mobile && bun vitest run -t "test name" src/components/Button/Button.test.tsx`
- Mobile Tauri test: `cd mobile && cd src-tauri && cargo test command_works -- --exact`

## Code Style Guidelines

### Rust (API + Tauri)

- Use `async fn`, `Result<T, E>`, `Option<T>`, and `?` for fallible paths.
- Avoid `.unwrap()`/`.expect()` except in tests or startup bootstrapping.
- Add explicit error context with `anyhow` when crossing IO/DB boundaries.
- Use `thiserror` for API-facing error enums.
- Keep handlers thin; move logic to services or helper functions.

### Rust Import Order

1. `use std::...`
2. `use tokio::...`
3. External crates
4. Internal modules
5. `use crate::...`

### Rust Naming

- Structs/enums: PascalCase (`FeedResult`)
- Functions/modules: snake_case (`refresh_all_feeds`)
- Constants: SCREAMING_SNAKE_CASE (`MAX_FEED_BYTES`)
- Route handlers: verb-first (`get_feeds`, `create_feed`)

### Rust Error Handling

- Return JSON errors with `error` and `status` fields.
- Status codes: 400/401/404/409/422/500 as appropriate.
- Log internal errors with context, but return generic messages to clients.
- Avoid leaking DB or network errors in responses.

### TypeScript + SolidJS

- Use explicit types; avoid implicit `any`.
- Define interfaces for props and data structures.
- Components use `Component<Props>`.
- Never destructure props; access as `props.name`.
- Signals use getter/setter pattern; do not destructure.
- Prefer `createMemo` for derived state and `createResource` for async data.
- Use Solid control-flow components (`<For>`, `<Show>`, `<Switch>`, `<Match>`) instead of `.map()` in JSX.
- Use `createStore` for collections; `batch()` for grouped updates.
- Lazy-load routes with `lazy(() => import(...))`.

### TypeScript Import Order

1. External packages (SolidJS, Tauri, etc.)
2. Internal modules via `@/` alias
3. Relative imports

### TypeScript Naming & Files

- Components: PascalCase (`UserList`)
- Hooks: camelCase with `use` prefix (`useTheme`)
- Stores: singular noun (`userStore`)
- Component files: kebab-case (`user-list.tsx`)
- Utilities: camelCase (`api.ts`)
- Props interfaces: PascalCase with `Props` suffix (`ButtonProps`)

### Formatting & Config

- Follow existing formatting in the file being edited.
- TypeScript is `strict` with `noUnusedLocals` and `noUnusedParameters`.
- Module resolution uses `bundler`; prefer ESM-style imports.
- Use `@/` alias for `src/*` paths when appropriate.

### Tauri-Specific

- Use capability-based permissions in `mobile/src-tauri/capabilities/`.
- Always include `$schema` in capability files.
- Validate all command inputs and return meaningful errors.
- Use Tauri state for app-wide resources.
- Use `tokio::task::spawn_blocking` for CPU-heavy work.

## Security

- Use constant-time comparison for API keys.
- Validate inputs early, especially URLs, IDs, and file paths.
- Reject unsafe or local network URLs in feeds.
- Do not log secrets or full authorization headers.
- Keep CSP up to date in `mobile/src-tauri/tauri.conf.json`.

## Key Resources

- Architecture: `PLAN.md`
- API agent guide: `api/AGENTS.md`
- Mobile agent guide: `mobile/AGENTS.md`
- Use Axum, Tauri, or SolidJS skills for deeper patterns.
