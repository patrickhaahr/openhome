# PRD: Biometric-Gated API Key Access (Mobile)

## Overview
Protect access to the stored API key with biometric authentication on every launch and resume. The API key never leaves Rust memory, is encrypted at rest, and is cleared on background/lock events.

## Goals
- Require biometric authentication before the API key is used.
- Keep the API key out of the webview and JS state entirely.
- Encrypt the API key at rest using ChaCha20-Poly1305.
- Clear decrypted key material on background/lock and app exit.
- Support Android and iOS with a consistent resume flow.

## Non-goals
- Data migration from legacy plaintext keyring entries.
- Desktop biometric support.
- Long-term key rotation or recovery.

## User Flows
1. First-time setup
   - User enters API key.
   - App prompts biometric auth.
   - API key is encrypted and saved; key is cached in Rust memory.
2. App launch with existing key
   - App checks `ApiKeyStatus`.
   - If `Locked`, show unlock screen and prompt biometric.
3. Resume from background
   - App locks on background.
   - On resume, prompt biometric and unlock.

## Architecture

### Storage Model
- Keyring entries:
  - `master_key` (base64 32-byte random key)
  - `api_key_encrypted` (JSON payload with nonce + ciphertext)
- The API key is only stored in encrypted form at rest.
- The decrypted API key lives only in Rust memory (`ConfigState.api_key`).

### Crypto Model
- Algorithm: ChaCha20-Poly1305.
- Key: 32 bytes, generated once at startup and stored as `master_key`.
- Nonce: 12 bytes, generated via `OsRng` for each encryption.
- Payload format (JSON):
  - `version: 1`
  - `nonce: base64`
  - `ciphertext: base64`
- Zeroize plaintext buffers after use.

### Biometric Gate
- Use `@tauri-apps/plugin-biometric` and `tauri-plugin-biometric`.
- Biometric is a gate only; all encryption/decryption occurs in Rust.
- Auth options: `allowDeviceCredential: true` plus platform-specific titles.

### Lifecycle Handling
- Rust emits `auth:resume` on app resume.
  - Use `RunEvent::Resumed` (documented) and also handle `RunEvent::Reopen` if emitted.
- Rust clears cache on `RunEvent::Exit`.
- Frontend listens for `visibilitychange` and locks after a 500ms debounce.
- Optional: also lock on `WindowEvent::Focused(false)` if supported.

## Backend Spec (Rust)

### New/Updated Commands
- `set_api_key(key: String) -> Result<()>`
  - biometric auth -> encrypt -> store -> cache in memory.
- `unlock_and_cache_api_key() -> Result<()>`
  - biometric auth -> decrypt -> cache in memory.
- `clear_api_key_cache() -> Result<()>`
  - zeroize and clear memory cache.
- `get_api_key_status() -> ApiKeyStatus`
  - returns `NotSet | Locked | Unlocked`.
- `call_api(...)`
  - use only cached key (or debug override).
  - return `MissingApiKey` if locked.

### Master Key Initialization
- On app startup, ensure `master_key` exists in keyring.
- Generate once to avoid race conditions (no biometric gate on creation).

### ApiKeyStatus Enum
```rust
#[derive(Serialize)]
pub enum ApiKeyStatus {
    NotSet,
    Locked,
    Unlocked,
}
```

## Frontend Spec (SolidJS)

### Auth State
- Add `src/stores/auth.ts` with `ApiKeyStatus` state and `unlock()`.
- State is derived from `get_api_key_status`.
- API key is never stored in JS state.

### Screens
- `ApiKeySetup` uses `set_api_key` and does not prefill saved key.
- New `ApiKeyUnlock` screen prompts biometric and calls `unlock_and_cache_api_key`.
- `App` routes based on `ApiKeyStatus`:
  - `NotSet` -> setup
  - `Locked` -> unlock
  - `Unlocked` -> main UI

### Locking Behavior
- Debounce `document.visibilitychange` by 500ms to call `clear_api_key_cache`.
- On `auth:resume`, auto-prompt unlock.

## Permissions and Config
- Add biometric permission to `mobile/src-tauri/capabilities/mobile.json`:
  - `"biometric:default"`
- Add `NSFaceIDUsageDescription` to `mobile/src-tauri/Info.ios.plist`.
- Keep desktop capabilities unchanged.

## Dependencies
- Plugin install: `bun tauri add biometric`.
- Rust deps via package manager (no manual Cargo.toml edits):
  - `cargo add chacha20poly1305`
  - `cargo add rand`
  - `cargo add base64`
  - `cargo add zeroize --features zeroize_derive`
- JS deps via package manager if needed:
  - `bun add @tauri-apps/plugin-biometric`

## Error Handling
- Biometric unavailable -> show blocking error and keep locked.
- Biometric cancel/fail -> stay locked; do not cache key.
- Keyring errors -> return `KeyringUnavailable`.

## Security Considerations
- Never log plaintext keys or decrypted payloads.
- Zeroize all plaintext buffers (API key, master key copies).
- Only cache keys in Rust memory, never in JS.

## Testing
- Unit tests for encrypt/decrypt roundtrip and nonce length.
- Unit test for invalid payload handling.
- Manual QA:
  - Fresh install -> set key -> biometric success -> API calls work.
  - Background -> resume -> biometric required.
  - Biometric fail -> API calls blocked.
  - App exit -> cache cleared.

## Skills
- Tauri 
- SolidJS

## Tasks
- [ ] Add Rust crypto deps via `cargo add` in `mobile/src-tauri`.
- [ ] Create `mobile/src-tauri/src/crypto.rs` with ChaCha20-Poly1305 helpers.
- [ ] Initialize `master_key` at startup in `mobile/src-tauri/src/lib.rs`.
- [ ] Add `set_api_key` command with biometric gate and encryption.
- [ ] Add `unlock_and_cache_api_key` command with biometric gate and decryption.
- [ ] Add `clear_api_key_cache` command with zeroize.
- [ ] Add `ApiKeyStatus` enum and `get_api_key_status` command.
- [ ] Update `call_api` to rely only on cached key.
- [ ] Remove frontend API key loading from `mobile/src/stores/config.ts`.
- [ ] Add `mobile/src/stores/auth.ts` with status + unlock actions.
- [ ] Add `mobile/src/pages/api-key-unlock.tsx` UI.
- [ ] Route in `mobile/src/app.tsx` based on `ApiKeyStatus`.
- [ ] Add 500ms debounce lock on `visibilitychange` in `mobile/src/app.tsx`.
- [ ] Emit `auth:resume` on `RunEvent::Resumed` (and `RunEvent::Reopen` if emitted).
- [ ] Add `biometric:default` to `mobile/src-tauri/capabilities/mobile.json`.
- [ ] Add `NSFaceIDUsageDescription` to `mobile/src-tauri/Info.ios.plist`.
- [ ] Add Rust unit tests for crypto helpers.
- [ ] Run `cd mobile/src-tauri && cargo test`.

## References
- https://v2.tauri.app/plugin/biometric/
- https://v2.tauri.app/reference/javascript/biometric/
- https://docs.rs/chacha20poly1305
