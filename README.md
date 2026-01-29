# openhome

<p align="center">
  <img src="assets/app-icon.png" alt="OpenHome App Icon" width="150">
  <br><br>
  Homelab control app with a Rust Axum API and Tauri v2 mobile frontend.
  <br><br>
  <strong>Tech Stack:</strong> Tauri v2 | Rust | Axum | SolidJS | TypeScript | SQLite
  <br><br>
  <img src="assets/architecture.png" alt="Architecture">
</p>

## Features

- **Docker Management** - Control containers on your homelab
- **AdGuard Home** - Router-level DNS filtering
- **Feed Reader** - RSS/Atom feed aggregation
- **Secure** - API key stored in system keyring, not plaintext

## Screenshots

| Screen 1 | Screen 2 | Screen 3 |
|:-|:-|:-|
| ![Screen 1](assets/screen1.png) | ![Screen 2](assets/screen2.png) | ![Screen 3](assets/screen3.png) |

## Development

### API (Rust Axum)

```sh
cd api && cargo run          # Dev server
cd api && cargo test         # Run tests
cd api && cargo clippy       # Lint
```

### Mobile (Tauri + SolidJS)

```sh
cd mobile && bun install     # Install dependencies
cd mobile && bun run tauri android dev --host <IP> 
```

## Project Structure

```
openhome/
├── api/           # Rust Axum backend
├── mobile/        # Tauri v2 + SolidJS frontend
```

## Performance

| Metric       | Value   |
|--------------|---------|
| App Size     | ~20 MB  |
| Memory Usage | ~35 MB  |
| Startup Time | <0.5s   |
