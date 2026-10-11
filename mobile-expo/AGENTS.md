# Mobile guidance

- Before using Expo APIs, consult the documentation matching the installed version in `package.json`.
- Keep domain logic pure in `src/domain/`, state and effect sequencing in `src/application/`, adapters in `src/infrastructure/`, and rendering in `src/ui/`.
- Route device and service access through the Axum API; keep bridge and LAN integration on the server.
