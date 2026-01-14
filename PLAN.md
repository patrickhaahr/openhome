# **Full Architecture Spec: Homelab Mobile App**

## **1. System Overview**

A Tauri v2 mobile app (Android/iOS) that serves as a unified control interface for your homelab. The app connects via Tailscale to a Rust axum API running on your Raspberry Pi, which acts as a secure gateway to Docker, IR devices, and AdGuard Home.

---

## **2. Project Structure**

```
homelab-app/
├── api/                          # Rust axum backend
│   ├── Cargo.toml
│   ├── .env
│   └── src/
│       ├── main.rs               # Server setup, middleware
│       ├── auth.rs               # API key validation
│       ├── docker.rs             # Docker daemon client
│       ├── adguard.rs            # AdGuard Home client
│       ├── routes/
│       │   ├── mod.rs
│       │   ├── health.rs         # Docker health endpoints
│       │   ├── ir.rs             # IR control endpoints
│       │   ├── events.rs         # Event API proxy
│       │   └── adguard.rs        # AdGuard control endpoints
│       └── models/
│           ├── mod.rs
│           ├── docker.rs         # Container status structs
│           ├── ir.rs             # IR command structs
│           └── adguard.rs        # AdGuard structs
├── mobile/                       # Tauri v2 + SolidJS frontend
│   ├── src-tauri/
│   │   ├── Cargo.toml
│   │   ├── tauri.conf.json
│   │   ├── capabilities/
│   │   │   └── mobile.json       # Mobile permissions
│   │   └── src/
│   │       ├── lib.rs            # Tauri commands, auth interceptor
│   │       └── error.rs          # Error handling
│   ├── src/
│   │   ├── App.tsx               # Root component + routing
│   │   ├── main.tsx              # SolidJS mount
│   │   ├── stores/               # Global state management
│   │   │   ├── index.ts
│   │   │   ├── api.ts            # API config & key storage
│   │   │   ├── docker.ts         # Docker health state
│   │   │   ├── todos.ts          # Local todo persistence
│   │   │   ├── events.ts         # Events data & artist list
│   │   │   ├── remote.ts         # IR command state
│   │   │   └── adguard.ts        # AdGuard state
│   │   ├── routes/               # View components
│   │   │   ├── index.tsx         # Dashboard
│   │   │   ├── Todos.tsx         # Todo list view
│   │   │   ├── Health.tsx        # Docker health details
│   │   │   ├── Events.tsx        # Events view
│   │   │   ├── Remote.tsx        # IR remote view
│   │   │   ├── AdGuard.tsx       # AdGuard control view
│   │   │   └── Settings.tsx      # API key & config
│   │   ├── components/           # Reusable UI components
│   │   │   ├── ServiceCard.tsx
│   │   │   ├── IRButton.tsx
│   │   │   ├── EventCard.tsx
│   │   │   ├── AdGuardToggle.tsx
│   │   │   └── Toast.tsx
│   │   ├── utils/
│   │   │   ├── http.ts           # HTTP client wrapper
│   │   │   └── constants.ts      # App constants
│   │   └── types/
│   │       └── index.ts          # TypeScript interfaces
│   └── index.html
├── compose.yml                   # RPi API deployment
└── README.md
```

---

## **3. RPi API (Rust axum) Spec**

### **3.1 Server Configuration**
- **Bind Address**: `0.0.0.0:8000`
- **Docker Socket**: `unix:///var/run/docker.sock`
- **AdGuard Config**: `~/.config/homelab-api/adguard.toml` (chmod 600)
- **CORS**: Allow Tailscale IP range (e.g., `100.0.0.0/8`)
- **Logging**: `tracing` with `INFO` level

### **3.2 Authentication Middleware**
- **Header**: `X-API-Key: <key>`
- **Validation**: Constant-time comparison against `API_KEY` env var
- **Error**: `401 Unauthorized` on mismatch

### **3.3 Routes & Data Models**

#### **Health Routes** (`/api/health`)
```rust
// GET /api/health
// Returns: Vec<ContainerStatus>
[
  {
    "name": "jellyfin",
    "status": "running",  // "running" | "stopped" | "restarting"
    "uptime_seconds": 8100,
    "image": "jellyfin/jellyfin:latest",
    "ports": ["8096:8096/tcp"]
  }
]

// GET /api/health/:service_name
// Returns: Single ContainerStatus
```

**Implementation**: Use `bollard` crate to list containers with label `com.docker.compose.project=homelab`. Cache results for 5s.

---

#### **IR Control Routes** (`/api/ir`)
```rust
// POST /api/ir/tv/power
{ "action": "on" | "off" }
// Returns: { "success": true }

// POST /api/ir/tv/volume
{ "action": "up" | "down", "steps": 5 }
// Returns: { "success": true }

// POST /api/ir/speaker/mode
{ "mode": "opt" | "bluetooth" }
// Returns: { "success": true }

// POST /api/ir/speaker/volume
{ "action": "up" | "down", "steps": 5 }
// Returns: { "success": true }
```

**Implementation**: Each endpoint spawns `curl` to call pre-configured endpoints on RPi (e.g., `http://localhost:5000/ir/tv/power`). IR codes and device endpoints stored in `~/.config/homelab-api/ir-config.toml`.

---

#### **AdGuard Routes** (`/api/adguard`)
```rust
// GET /api/adguard/status
// Returns: { "enabled": true, "version": "v0.107.43" }

// POST /api/adguard/toggle
{ "enabled": false }
// Returns: { "success": true }

// POST /api/adguard/pause
{ "duration_minutes": 5 }
// Returns: { "success": true, "resumes_at": "2026-01-14T13:05:00Z" }
```

**Implementation**: 
- Use `reqwest` to call AdGuard Home API on GLinet router (`http://<router-tailscale-ip>:3000`)
- Auth: Basic auth from `adguard.toml`
- Pause: Spawn async task with `tokio::time::sleep`, then re-enable

---

#### **Events Proxy** (`/api/events`)
```rust
// GET /api/events?artists=artist1,artist2&location=Copenhagen,Denmark
// Returns: Vec<Event>
[
  {
    "artist": "Artist Name",
    "venue": "Vega",
    "city": "Copenhagen",
    "date": "2026-02-15",
    "ticket_url": "https://..."
  }
]
```

**Implementation**:
- Concurrently query Bandsintown + Ticketmaster APIs
- Your API keys stored in `~/.config/homelab-api/events.toml`
- Merge, sort by date, deduplicate
- Cache for 1 hour

---

## **4. Mobile App Architecture**

### **4.1 Tauri Configuration**
**Plugins** (`src-tauri/Cargo.toml`):
```toml
tauri-plugin-store = "2.0"        # Encrypted API key storage
tauri-plugin-http = "2.0"         # HTTP client
tauri-plugin-notification = "2.0" # Toast notifications
tauri-plugin-fs = "2.0"           # Todo list file storage
tauri-plugin-shell = "2.0"        # Open ticket URLs
```

**Capabilities** (`capabilities/mobile.json`):
```json
{
  "identifier": "mobile-capability",
  "platforms": ["android", "iOS"],
  "permissions": [
    "store:allow-set",
    "store:allow-get",
    "http:allow-fetch",
    "notification:allow-show",
    "fs:allow-appdata-read",
    "fs:allow-appdata-write",
    "shell:allow-open"
  ]
}
```

**Security**: Enable `dangerousUseHttpScheme` for Tailscale IPs in `tauri.conf.json`.

---

### **4.2 Store Architecture (SolidJS)**

**`api.ts`** - Global API config
```typescript
interface ApiConfig {
  baseUrl: string;        // e.g., "http://phi:8000" or "http://100.100.100.10:8000"
  key: string | null;     // Stored encrypted
  isConfigured: boolean;
}
// Actions: setBaseUrl, setKey, validateConnection
```

**`docker.ts`** - Docker health state
```typescript
interface ContainerStatus {
  name: string;
  status: 'running' | 'stopped' | 'restarting';
  uptimeSeconds: number;
  image: string;
  ports: string[];
}
// Polls /api/health every 30s, shows toast on status change
```

**`todos.ts`** - Local todo persistence
```typescript
interface Todo {
  id: string;
  text: string;
  completed: boolean;
  createdAt: string;
}
// Stored in: ~/.local/share/com.homelab.app/todos.json
// Actions: add, toggle, delete, reorder
```

**`events.ts`** - Events & artists
```typescript
interface Event { /* ... */ }
interface EventsState {
  artists: string[];      // User-configured artist list
  events: Event[];
  lastUpdated: string | null;
}
// Actions: addArtist, removeArtist, refreshEvents
```

**`remote.ts`** - IR command queue
```typescript
interface IRCommand {
  endpoint: string;
  body: Record<string, unknown>;
}
// Debounces commands, shows loading state, handles errors
```

**`adguard.ts`** - AdGuard state
```typescript
interface AdGuardStatus {
  enabled: boolean;
  version: string;
  pausedUntil: string | null;  // ISO timestamp
}
// Polls status on app open, shows countdown timer
```

---

### **4.3 Routing & Views**

**Dashboard** (`/`)
- Grid of service cards (Jellyfin, Vaultwarden, Caddy) with status badges
- Quick action buttons: Pause AdGuard 5min, TV Power, Refresh Events
- Todo widget (last 3 items + "View All")

**Todos** (`/todos`)
- Full-screen todo list
- Add/delete/toggle with smooth animations
- Drag-to-reorder

**Health** (`/health`)
- Detailed container list with uptime, ports, image versions
- Pull-to-refresh
- Tap container for logs (future feature placeholder)

**Events** (`/events`)
- Search bar to add artists
- Horizontal date sections
- Tap event to open ticket URL in browser

**Remote** (`/remote`)
- TV section: Power, Volume ±
- Speaker section: Mode toggle (OPT/Bluetooth), Volume ±
- Visual feedback: Button press animations, loading spinners

**AdGuard** (`/adguard`)
- Large toggle switch
- Pause buttons: 5min, 10min, 30min, 1hr
- Status text: "Active", "Paused until 13:05", "Resuming in 4:23"

**Settings** (`/settings`)
- API key input (masked)
- Base URL input (with Tailscale IP helper)
- Artist list editor
- "Test Connection" button with toast feedback

---

### **4.4 Component Hierarchy**

**Shared Components**:
- `Toast.tsx`: Top-right notifications, auto-dismiss 3s
- `IRButton.tsx`: Debounced button with ripple effect, loading state
- `ServiceCard.tsx`: Status indicator, uptime, tap for details
- `EventCard.tsx`: Artist, venue, date, ticket link
- `AdGuardToggle.tsx`: iOS-style toggle switch

---

## **5. Network & Security Model**

### **5.1 Tailscale Setup**
- **RPi**: `phi` (100.100.100.10)
- **GLinet Router**: `flint2` (100.100.100.5)
- **Mobile**: Dynamic Tailscale IP
- **ACLs**: In Tailscale admin, restrict mobile → RPi → router access

### **5.2 Authentication Flow**
1. User inputs API key in Settings
2. Tauri stores it encrypted in `~/.local/share/com.homelab.app/store.bin`
3. HTTP client interceptor adds `X-API-Key` header to all requests
4. RPi API validates against `API_KEY` env var

### **5.3 Environment Configs**
**RPi API `.env`**:
```bash
API_KEY="your-secret-key-here"
DOCKER_SOCKET_PATH="/var/run/docker.sock"
ADGUARD_CONFIG_PATH="/home/ph/.config/homelab-api/adguard.toml"
IR_CONFIG_PATH="/home/ph/.config/homelab-api/ir-config.toml"
EVENTS_CONFIG_PATH="/home/ph/.config/homelab-api/events.toml"
```

**Mobile Dev Config**:
```typescript
// In stores/api.ts, dev mode:
const DEV_BASE_URL = "http://100.100.100.10:8000";
```

---

## **6. Build & Deployment**

### **6.1 RPi API Deployment**
```yaml
# compose.yml
services:
  homelab-api:
    build: ./rpi-api
    container_name: homelab-api
    restart: unless-stopped
    ports:
      - "8000:8000"
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock
      - /home/ph/.config/homelab-api:/config
    environment:
      - API_KEY=${API_KEY}
      - RUST_LOG=info
    network_mode: host  # For Tailscale access
```

**Deploy**:
```bash
cd rpi-api
docker compose up -d --build
```

### **6.2 Mobile Build Commands**
**Android**:
```bash
cd mobile
bun tauri android init
bun tauri android dev        # USB debugging
bun tauri android build --apk # Release APK at:
# src-tauri/gen/android/app/build/outputs/apk/release/
```

**iOS** (requires macOS):
```bash
bun tauri ios init
bun tauri ios build
```

---

## **7. Development Setup Guide**

### **7.1 Initial Setup (RPi)**
```bash
# 1. Install Rust on RPi
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 2. Create project
cargo new --bin rpi-api
cd rpi-api

# 3. Add dependencies
cargo add axum tokio -F full
cargo add bollard reqwest serde serde_json tracing tracing-subscriber
cargo add tower-http -F cors

# 4. Create config dir
mkdir -p ~/.config/homelab-api
chmod 700 ~/.config/homelab-api

# 5. Create adguard.toml
cat > ~/.config/homelab-api/adguard.toml <<EOF
url = "http://100.100.100.5:3000"
username = "admin"
password_hash = "..."  # From /etc/AdGuardHome/config.yaml on router
EOF

# 6. Create ir-config.toml
cat > ~/.config/homelab-api/ir-config.toml <<EOF
[tv]
power_on_url = "http://localhost:5000/ir/tv/power/on"
power_off_url = "http://localhost:5000/ir/tv/power/off"
volume_up_url = "http://localhost:5000/ir/tv/volume/up"
volume_down_url = "http://localhost:5000/ir/tv/volume/down"

[speaker]
opt_mode_url = "http://localhost:5000/ir/speaker/mode/opt"
bluetooth_mode_url = "http://localhost:5000/ir/speaker/mode/bluetooth"
volume_up_url = "http://localhost:5000/ir/speaker/volume/up"
volume_down_url = "http://localhost:5000/ir/speaker/volume/down"
EOF

# 7. Create events.toml
cat > ~/.config/homelab-api/events.toml <<EOF
bandsintown_key = "your-key"
ticketmaster_key = "your-key"
EOF

# 8. Set API key
export API_KEY="your-secret-key"

# 9. Run
cargo run --release
```

### **7.2 Initial Setup (Mobile)**
```bash
# 1. Install Tauri CLI
bun add -g @tauri-apps/cli

# 2. Create project
bun create tauri-app --template solid-ts homelab-app

# 3. Add plugins
cd mobile/src-tauri
cargo add tauri-plugin-store tauri-plugin-http tauri-plugin-notification tauri-plugin-fs tauri-plugin-shell

# 4. Configure capabilities (see 4.1)

# 5. Dev
bun tauri android dev
```

---

## **8. Future Enhancements (Out of Scope)**
- Container logs viewer
- Resource usage graphs (CPU/memory)
- Wake-on-LAN for devices
- Home Assistant integration
- Push notifications for events

---

**Ready to start implementation?** Begin with RPi API's health endpoint, then Tauri API key storage flow.
