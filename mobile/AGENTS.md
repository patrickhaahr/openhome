# AGENTS.md - Mobile App Agent Guide

This file contains mobile-specific guidelines. General project guidelines are in `../AGENTS.md`.

## Project Snapshot

- Frontend: SolidJS 1.9 + TypeScript 5.6
- Backend: Tauri v2 (Rust, in `src-tauri/`)
- Build: Vite 6 with bun
- UI: Tailwind CSS 4, shadcn (via `@/` alias)

## Testing

- Frontend tests are not configured yet. When added, use Vitest.
- Run a single frontend test:
  - `bun vitest run --reporter=verbose src/components/Button/Button.test.tsx`
  - `bun vitest run -t "test name" src/components/Button/Button.test.tsx`

## Error Handling

- Handle async errors explicitly with `try/catch`
- Log with context before returning or rethrowing
- Never swallow errors or return empty catch blocks

## Available Skills

- Use `solidjs` skill for SolidJS reactivity patterns and component patterns
- Use `tauri` skill for Tauri v2 security, plugin architecture, and build optimization
