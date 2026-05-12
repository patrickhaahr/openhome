# Native Android Migration Plan

## Scope

- Rebuild the mobile client natively in `mobile-native/`
- Keep the Axum API contract unchanged
- Keep the existing `Setup Flow` concept: configure `Base URL` and `API Key`, validate with `GET /api/health`
- Do not keep the old launch-time `Unlock Flow` in `mobile-native`
- Initial native release shows only the `Home Tab` and `Remote` tab

## Vertical Slices

### 1. Setup Flow and API health validation

- Type: AFK
- Blocked by: None
- Status: Done

#### What to build

Build the native `Setup Flow` for `mobile-native` so a user can enter a `Base URL` and `API Key`, validate them against `GET /api/health`, persist the working configuration locally, and enter the app without a launch-time lock screen.

#### Acceptance criteria

- [x] A fresh install opens into the native `Setup Flow`
- [x] The app validates the entered `Base URL` and `API Key` with `GET /api/health` using `Authorization: Bearer <API_KEY>`
- [x] Invalid configuration shows a clear error and does not enter the app
- [x] Valid configuration is persisted locally and the next app launch skips setup
- [x] After successful setup, the app opens directly into the initial tab shell with `Home` selected

### 2. Initial tab shell and authorized client

- Type: AFK
- Blocked by: None
- Status: Done

#### What to build

Build the initial native app shell with the `Home Tab` and `Remote` tab, backed by one authorized client layer that reads the stored configuration and applies the bearer `API Key` to all Axum API requests.

#### Acceptance criteria

- [x] After setup, the app shows exactly two tabs: `Home` and `Remote`
- [x] `Home` is the default selected tab on app start
- [x] The shared client reads the stored `Base URL` and `API Key` for every API request
- [x] The native client does not show a launch-time `Unlock Flow`
- [x] A missing or invalid stored configuration returns the user to the `Setup Flow`

### 3. IR status preload from Home

- Type: AFK
- Blocked by: None
- Status: Ready

#### What to build

Build a shared IR state source that starts from `Home` by calling `GET /api/ir`, stores the returned status and `available_commands`, and feeds both the `Home Tab` and the `Remote` tab.

#### Acceptance criteria

- [ ] The `Home Tab` triggers `GET /api/ir` when the authorized app opens
- [ ] The app stores the result in one shared IR state source used by both `Home` and `Remote`
- [ ] The `Remote` tab retries `GET /api/ir` automatically when entered after a failed preload
- [ ] Manual retry is available when IR status fails to load
- [ ] IR buttons remain disabled until `GET /api/ir` succeeds
- [ ] Commands missing from `available_commands` remain visible but disabled

### 4. Home Remote Controls

- Type: AFK
- Blocked by: 3

#### What to build

Add `Home Remote Controls` to the `Home Tab` for the v1 quick actions `bluetooth` and `optical`, using direct `POST /api/ir/send` calls against the shared authorized client.

#### Acceptance criteria

- [ ] The `Home Tab` shows exactly two quick controls: `bluetooth` and `optical`
- [ ] Tapping a quick control sends `POST /api/ir/send` with JSON `{ "command": "..." }`
- [ ] Only the tapped control is blocked while its request is in flight
- [ ] Successful sends do not show a success message
- [ ] Failed sends show an action-level error without invalidating the full IR state
- [ ] The app does not auto-refresh `GET /api/ir` after a successful send

### 5. Remote tab v1 IR Remote

- Type: AFK
- Blocked by: 3

#### What to build

Build the dedicated `Remote` tab as a fixed client-owned `IR Remote` using the v1 `Remote Button Set`: `power`, `bluetooth`, `optical`, `mute`, `volume-up`, and `volume-down`.

#### Acceptance criteria

- [ ] The `Remote` tab shows the full v1 `Remote Button Set`
- [ ] Each `Remote Button` maps to the backend command name as its canonical ID
- [ ] The UI can use client-facing labels or icons without changing the backend command IDs
- [ ] Unavailable commands remain visible but disabled based on shared IR state
- [ ] Only the tapped button is blocked while its send request is in flight
- [ ] Successful sends are silent and failed sends stay action-local

### 6. In-app reconfiguration

- Type: AFK
- Blocked by: 2

#### What to build

Add an in-app way to reopen configuration, change the stored `Base URL` and `API Key`, revalidate them with `GET /api/health`, and switch the running client over to the new working configuration.

#### Acceptance criteria

- [ ] A user can reopen configuration after initial setup
- [ ] Saving changes revalidates the new `Base URL` and `API Key` with `GET /api/health`
- [ ] Invalid replacement settings are rejected and the previous working configuration remains active
- [ ] Valid replacement settings become the active configuration for future API calls
- [ ] After successful reconfiguration, the app returns to the authorized tab shell

## Notes

- This plan is intentionally clientside-only
- The backend Axum endpoints, request shapes, and auth contract remain unchanged
- Future tabs like Docker, AdGuard, and Feeds should be added as separate vertical slices after the IR milestone is working end-to-end
