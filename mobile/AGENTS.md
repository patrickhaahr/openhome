# AGENTS.md - Guidelines for AI Coding Agents

This file provides guidelines for AI agents operating in this SolidJS + Tauri v2 project.

## Project Overview

This is a cross-platform desktop/mobile application built with:
- **Frontend:** SolidJS 1.9+ with TypeScript 5.6+
- **Backend:** Tauri v2 (Rust)
- **Build Tool:** Vite 6
- **Package Manager:** bun

## Domain-Specific Guidelines

When working in specific areas of the codebase, consult the dedicated AGENTS.md files:
- **SolidJS frontend**: `./src/AGENTS.md` - Frontend patterns, components, and SolidJS-specific conventions
- **Tauri backend**: `./src-tauri/AGENTS.md` - Rust backend patterns, Tauri commands, and security model

## Build Commands

| Command | Description |
|---------|-------------|
| `bun start` | Start Vite dev server |
| `bun run dev` | Run Tauri Android dev build |
| `bun run build` | Build Tauri bundle AND Vite frontend |
| `bun run serve` | Preview Vite production build |
| `bun run check` | Run Rust clippy linting (`cargo clippy -- -D warnings`) |

## Testing

This project does not currently have a test framework configured. When adding tests:
- Use **Vitest** for SolidJS components (matches Vite ecosystem)
- Place tests alongside components with `.test.tsx` extension
- Run single test: `bun vitest run --reporter=verbose src/components/Button/Button.test.tsx`

## Code Style Guidelines

### TypeScript Conventions

- Use explicit types over implicit inference
- Define interfaces for all props and data structures
- Use `Component<Props>` type for SolidJS components
- Create generic types for stores and resources

```typescript
import { Component } from "solid-js";

interface ButtonProps {
  label: string;
  onClick: () => void;
}

const Button: Component<ButtonProps> = (props) => {
  return <button onClick={props.onClick}>{props.label}</button>;
};
```

### SolidJS Specific Rules

**CRITICAL:** Components execute exactly once. Think "setup function" not "render function".

- Never destructure props - access as `props.name` not `{ name }`
- Use `createSignal` with getter/setter pattern - never destructure
- Use `createMemo` for expensive derived computations
- Use `createEffect` for side effects with `onCleanup`
- Use `createResource` for async data with Suspense
- Use `<For>`, `<Show>`, `<Switch>`, `<Match>` for control flow (never `.map()` in JSX)

```tsx
// CORRECT
function UserList(props) {
  const [users, setUsers] = createSignal<User[]>([]);
  const userCount = createMemo(() => users().length);
  
  createEffect(() => {
    console.log("Users updated:", users());
    onCleanup(() => cleanup());
  });
  
  return (
    <For each={users()}>
      {(user) => <div>{user.name}</div>}
    </For>
  );
}
```

### Tauri/Rust Conventions

- Use capability-based permissions in `src-tauri/capabilities/`
- Define granular permissions per window
- Always include `$schema` in capability files
- Use `tokio::task::spawn_blocking` for CPU-intensive operations
- Validate all command inputs, return meaningful error messages
- Use Tauri state for application-wide resources

```rust
#[tauri::command]
async fn read_file(path: String, state: tauri::State<AppState>) 
  -> Result<String, String> {
    if path.is_empty() {
      return Err("Path cannot be empty".to_string());
    }
    std::fs::read_to_string(path)
      .map_err(|e| format!("Failed to read: {}", e))
  }
```

### Import Order

1. External packages (SolidJS, Tauri, etc.)
2. Internal modules (components, hooks, stores, utils)
3. Relative imports

```typescript
import { createSignal, createEffect, Component } from "solid-js";
import { invoke } from "@tauri-apps/api/core";
import { useAuth } from "@/hooks/useAuth";
import { Button } from "./Button";
```

### Naming Conventions

- **Components:** PascalCase (`UserList`, `SettingsPanel`)
- **Hooks:** camelCase with `use` prefix (`useAuth`, `useTheme`)
- **Stores:** singular noun (`userStore`, `cartStore`)
- **Files:** kebab-case for components (`user-list.tsx`), camelCase for utilities (`api.ts`)
- **Types/Interfaces:** PascalCase with `Props` suffix for component props

### Error Handling

- Always handle async errors explicitly
- Log errors with context before returning
- Return meaningful error messages from Tauri commands
- Use `try/catch` for async operations
- Never swallow errors silently

### File Structure

```
src/
├── components/
│   ├── Button/
│   │   ├── Button.tsx
│   │   ├── Button.module.css
│   │   └── index.ts
│   └── index.ts
├── contexts/
├── hooks/
├── routes/
├── stores/
├── utils/
├── types.d.ts
└── app.tsx
src-tauri/
├── capabilities/
├── src/
└── tauri.conf.json
```

### Performance Guidelines

- Use stores (`createStore`) for collections, not individual signals
- Batch updates with `batch()` when modifying multiple signals
- Lazy load routes with `lazy(() => import(...))`
- Use pagination for large datasets in Tauri commands

## Security

- Follow Tauri capability model - define minimal permissions
- Validate all inputs to Tauri commands
- Configure CSP in `tauri.conf.json`
- Never expose sensitive data in error messages

## Key Takeaways

1. Components run once - set up reactivity, don't trigger re-renders
2. Props are reactive proxies - access as functions
3. Use control flow components (`<For>`, `<Show>`) not array methods
4. Explicit permissions for all Tauri operations
5. Always cleanup effects and subscriptions
6. Consult domain-specific 'Tauri' or 'SolidJS' skills for detailed patterns
