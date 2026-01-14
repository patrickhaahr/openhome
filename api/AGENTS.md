# AGENTS.md - Guidelines for AI Coding Agents

Rust axum API for homelab control (Docker, IR, AdGuard, plants) accessible via Tailscale.

## Build & Test Commands

| Command | Description |
|---------|-------------|
| `cargo build` | Build project |
| `cargo build --release` | Release build |
| `cargo run` | Development mode |
| `cargo clippy` | Linter (fix warnings) |
| `cargo clippy -- -D warnings` | Treat warnings as errors |
| `cargo fmt` | Format code |
| `cargo test` | Run all tests |
| `cargo test test_name -- --exact` | Run single test |

## Architecture (see PLAN.md)

**Route Priority:** 0) `/api/health` (no auth) → 1) Docker, IR → 2) AdGuard → 3) Plants

**Middleware:** Health bypasses auth → Validate `X-API-Key` → CORS (Tailscale IPs) → Logging

**Caching:** Docker: 5s, Plants: 60s, Health/IR/AdGuard: none (real-time)

## Code Style

**Rust Conventions:**
- Run `cargo fmt` and `cargo clippy -- -D warnings` before committing
- Use `async fn`, `Result<T, E>`, `Option<T>`, and `?` for errors
- Avoid `.unwrap()`/`.expect()` (tests/main only)
- Explicit error context: `.map_err(|e| Error::NetworkFailed(format!("Failed: {}", e)))?`

**Import Order:**
1. `use std::` → 2. `use tokio::` → 3. External crates → 4. Internal → 5. `use crate::`

```rust
use std::sync::Arc;
use tokio::sync::RwLock;
use axum::{extract::State, routing::get, Router};
use reqwest::Client;
use crate::error::Error;
```

**Naming:** PascalCase structs/enums, snake_case functions/modules, SCREAMING_SNAKE_CASE constants

**Error Handling:** Consistent JSON `{"error": "msg", "status": 401}` with status codes (401/404/422/502/503/504)

```rust
#[derive(Debug)]
pub enum Error { Unauthorized(String), NotFound(String), ServiceUnavailable(String) }

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let (status, msg) = match self {
            Error::Unauthorized(m) => (StatusCode::UNAUTHORIZED, m),
            Error::NotFound(m) => (StatusCode::NOT_FOUND, m),
            Error::ServiceUnavailable(m) => (StatusCode::SERVICE_UNAVAILABLE, m),
        };
        (status, Json(json!({"error": msg, "status": status.as_u16()}))).into_response()
    }
}
```

**Caching:** `Arc<RwLock<HashMap<String, (Instant, T)>>>` with read-lock check, write-lock update

```rust
async fn get_or_update<T>(cache: &Cache, key: &str, ttl: Duration, fetch: impl Fn() -> impl Future<Output = T>) -> T {
    if let Some((ts, val)) = cache.read().await.get(key) {
        if ts.elapsed() < ttl { return val.clone(); }
    }
    let val = fetch().await;
    cache.write().await.insert(key.to_string(), (Instant::now(), val.clone()));
    val
}
```

**Logging:** `tracing` with INFO (requests), WARN (transient failures), ERROR (persistent failures)

```rust
info!(method = %method, path = %path, status = %status, "Request completed");
```

**Security:** Constant-time API key comparison, validate inputs, env vars for secrets, no logging of secrets

## Project Structure

```
src/
├── main.rs          # Entry point, router setup
├── mod.rs           # Module declarations
├── docker.rs        # Docker container endpoints
├── adguard.rs       # AdGuard Home endpoints
├── plants.rs        # Plant sensor endpoints
├── ir.rs            # IR control endpoints
├── error.rs         # Error types and IntoResponse
└── config.rs        # App config, state
```

## Key Takeaways

1. Format and lint before committing
2. Implement routes by priority
3. Consistent error JSON format
4. Cache slow endpoints only
5. Structured logging with tracing
6. Follow Rust idioms (Result, ?, explicit context)
7. Modular code: `docker`, `adguard`, `plants`, `ir` modules
8. Use 'Axum' skill for comprehensive detailed patterns
