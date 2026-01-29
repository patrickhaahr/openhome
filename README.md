<h1 align="center">openhome</h1>

<p align="center">
  <img src="assets/app-icon.png" alt="OpenHome App Icon" width="150">
  <br><br>
  Homelab control app with a Rust Axum API and Tauri v2 mobile frontend.
  <br><br>
  <strong>Tech Stack:</strong> Tauri v2 | Rust | Axum | SolidJS | TypeScript | SQLite
  <br><br>
  <img src="assets/architecture.png" alt="Architecture" width="75%">
</p>

## Features

- **Docker Management** - Control containers on your homelab
- **AdGuard Home** - Router-level DNS filtering
- **Feed Reader** - RSS/Atom feed aggregation
- **Stronghold Encryption** - API key encrypted at rest using IOTA Stronghold
- **Biometric Auth** - Fingerprint/FaceID required to decrypt and access API key
- **Secure Session** - Decrypted API key cached in memory with automatic lock after timeout

## Screenshots

| API Key Setup | Home Dashboard | Docker Management |
|:-|:-|:-|
| ![API Key Setup](assets/screen1.png) | ![Home Dashboard](assets/screen2.png) | ![Docker Management](assets/screen3.png) |

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
