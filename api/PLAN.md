# **RPi API High-Level Architecture & Design (Revised)**

## **System Overview**

A **Rust axum** service that centralizes homelab control behind a single authenticated API. Mobile app connects via Tailscale to endpoints that proxy Docker, IR devices, AdGuard Home, and plant sensors.

---

## **Route Architecture & Priority**

**Priority 0 (Critical - API Health)**
- **`GET /api/health`** - API self-health check
  - Returns: `{"status": "healthy", "version": "0.1.0", "timestamp": "2026-01-14T12:00:00Z"}`
  - **No authentication required** - used by Docker/K8s health probes
  - Checks: SQLite connectivity, config file readability
  - Status codes: `200 OK` (healthy), `503 Service Unavailable` (unhealthy)

**Priority 1 (Core Features)**
- **`GET /api/docker`** - Docker container health
  - Returns array of container statuses (Jellyfin, Vaultwarden, Caddy)
  - Polls Docker socket, 5-second cache
  - Auth: Required

- **`POST /api/ir/tv/power`** - TV power control
  - Body: `{"action": "on" | "off"}`
  - Proxies to IR service at `http://localhost:5000`
  - Auth: Required

- **`POST /api/ir/tv/volume`** - TV volume
  - Body: `{"action": "up" | "down", "steps": 5}`
  - Auth: Required

- **`POST /api/ir/speaker/mode`** - Speaker mode toggle
  - Body: `{"mode": "opt" | "bluetooth"}`
  - Auth: Required

- **`POST /api/ir/speaker/volume`** - Speaker volume
  - Body: `{"action": "up" | "down", "steps": 5}`
  - Auth: Required

**Priority 2 (Important - AdGuard)**
- **`GET /api/adguard/status`** - AdGuard Home protection state
  - Returns: `{"enabled": true, "version": "v0.107.43"}`
  - Calls GLinet router at `http://100.100.100.5:3000`
  - Auth: Required

- **`POST /api/adguard/toggle`** - Enable/disable protection
  - Body: `{"enabled": false}`
  - Auth: Required

- **`POST /api/adguard/pause`** - Pause for duration
  - Body: `{"duration_minutes": 5}`
  - Spawns async task to re-enable after delay
  - Returns: `{"success": true, "resumes_at": "2026-01-14T13:05:00Z"}`
  - Auth: Required

**Priority 3 (Monitoring - Plants)**
- **`GET /api/plants`** - All plant statuses
  - Returns array with moisture levels, battery, temperature
  - Polls sensor endpoints, 60-second cache
  - Auth: Required

- **`GET /api/plants/:id`** - Single plant details
  - Returns: Status + 7-day moisture history from SQLite
  - Auth: Required

- **`POST /api/plants/:id/water`** - Trigger watering
  - Body: `{"duration_seconds": 10}` (optional)
  - Calls relay endpoint if configured
  - Auth: Required

- **`GET /api/plants/summary`** - Quick stats
  - Returns: `{"total_plants": 3, "needs_water": 1, "optimal": 2}`
  - Auth: Required

---

## **Component Flow Diagram**

```
┌─────────────────────────────────────────────────────────────┐
│ Mobile App (Tailscale)                                     │
│ Request: GET /api/health (No Auth)                         │
└──────────────────────┬──────────────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────────┐
│ axum Router                                                 │
│  ┌──────────────────────────────────────────────────────┐  │
│  │ Middleware Stack                                     │  │
│  │  1. Health route bypasses auth                       │  │
│  │  2. Other routes: AuthMiddleware validates X-API-Key │  │
│  │  3. CORS: Allow Tailscale IPs                        │  │
│  └──────────────────────────────────────────────────────┘  │
│                 │           │           │          │        │
│                 ▼           ▼           ▼          ▼        │
│           ┌─────────┐  ┌────────┐  ┌─────────┐  ┌──────┐  │
│           │ Docker  │  │  IR    │  │AdGuard  │  │Plant │  │
│           │ Client  │  │ Client │  │ Client  │  │Client│  │
│           └────┬────┘  └───┬────┘  └────┬────┘  └───┬──┘  │
│                │           │            │           │       │
│                │ Unix      │ HTTP       │ HTTP      │ HTTP  │
│                │ Socket    │            │           │       │
│                ▼           ▼            ▼           ▼       │
│         /var/run/    localhost:5000  100.100.100.5:3000 Sensors│
│         docker.sock   (IR Service)   (Router)       (Various) │
│                                                               │
│           ┌───────────────────────────────────────────────┐   │
│           │ SQLite (Plant History)                        │   │
│           │ ~/.local/share/homelab-api/plants.db          │   │
│           └───────────────────────────────────────────────┘   │
└───────────────────────────────────────────────────────────────┘
```

---

## **Configuration Management**

**File Structure**:
```
~/.config/homelab-api/
├── adguard.toml          # Router credentials
├── ir-config.toml        # IR endpoint mappings
└── plants.toml           # Sensor endpoints & thresholds

~/.local/share/homelab-api/
└── plants.db             # SQLite (moisture history)
```

**Environment Variables** (`.env`):
```bash
API_KEY="your-secret-key"
BIND_ADDRESS="0.0.0.0:8000"
CONFIG_DIR="/home/ph/.config/homelab-api"
DATA_DIR="/home/ph/.local/share/homelab-api"
DOCKER_SOCKET_PATH="/var/run/docker.sock"
COMPOSE_PROJECT_NAME="homelab"
ADGUARD_URL="http://100.100.100.5:3000"
ADGUARD_USERNAME="admin"
ADGUARD_PASSWORD_HASH="..."
IR_SERVICE_URL="http://localhost:5000"
```

---

## **Authentication & Security**

**API Key Flow**:
1. Mobile app stores key in Tauri secure storage
2. All requests (except `/api/health`) include `X-API-Key: <key>` header
3. Middleware performs constant-time comparison against `.env` `API_KEY`
4. On mismatch: 401 Unauthorized
5. On success: Request proceeds to route handler

**Tailscale Integration**:
- API binds to `0.0.0.0:8000` but only accessible via Tailscale network
- GLinet router's AdGuard Home runs on `http://100.100.100.5:3000` (Tailscale IP)
- IR service runs on RPi localhost (`http://localhost:5000`) - not exposed externally

---

## **Caching Strategy**

| Route | Cache TTL | Cache Type | Rationale |
|-------|-----------|------------|-----------|
| `/api/health` | None | N/A | Must be real-time for health probes |
| `/api/docker` | 5s | In-memory | Docker daemon is local, fast |
| `/api/plants` | 60s | In-memory | Sensors are slow, battery-powered |
| `/api/adguard/*` | None | N/A | State changes frequently |
| `/api/ir/*` | None | N/A | Commands must be real-time |

**Cache Implementation**: `std::sync::RwLock<HashMap<String, (Instant, Value)>>` in `AppState`

---

## **Error Handling Philosophy**

**Client Errors (4xx)**:
- `401 Unauthorized`: Missing or invalid API key
- `404 Not Found`: Plant ID not in config
- `422 Unprocessable`: Invalid command format

**Server Errors (5xx)**:
- `502 Bad Gateway`: IR service unreachable
- `503 Service Unavailable`: Docker socket unreachable or SQLite error
- `504 Gateway Timeout`: AdGuard router unreachable

**Response Format** (All errors):
```json
{
  "error": "Human-readable message",
  "status": 401
}
```

---

## **Deployment Model**

**Docker Compose**:
```yaml
services:
  homelab-api:
    build: ./rpi-api
    ports: ["8000:8000"]
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock
      - ~/.config/homelab-api:/config
      - ~/.local/share/homelab-api:/data
    environment:
      - API_KEY=${API_KEY}
    network_mode: host
```

**Update Flow**:
```bash
cd ~/homelab-app/rpi-api
git pull
docker compose up -d --build
```

---

## **Monitoring & Observability**

**Health Probes**:
- **Liveness**: `GET /api/health` (no auth)
- **Readiness**: Check Docker socket + SQLite connectivity on startup

**Logging Levels**:
- `INFO`: Successful requests (method, route, status, duration)
- `WARN`: Transient failures (sensor timeout, IR service retry)
- `ERROR`: Persistent failures (Docker socket down, config missing)

**Key Metrics to Track**:
- Request rate per route
- Sensor response times (p95)
- Cache hit rates
- AdGuard pause durations

---

## **Future Extension Points**

1. **WebSocket Support**: Push plant moisture alerts in real-time
2. **GraphQL Layer**: Single endpoint for mobile to query exactly what it needs
3. **Plugin System**: Dynamic route loading for custom integrations
4. **Multi-User**: JWT auth with per-user plant configs
5. **Metrics Endpoint**: Prometheus metrics at `/metrics`

---

**Implementation Priority Order**:
1. **Core**: Auth middleware, `/api/health`, `/api/docker`
2. **IR Control**: `/api/ir/*` routes
3. **AdGuard**: `/api/adguard/*` routes
4. **Plants**: `/api/plants/*` routes + SQLite setup
5. **Polish**: Logging, error handling, CORS config
