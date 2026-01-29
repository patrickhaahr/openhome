pub mod config;
pub mod error;

use error::AppError;
use iota_stronghold::procedures::Runner;
use iota_stronghold::{Client, KeyProvider, Location, SnapshotPath, Stronghold};
use reqwest::{Client as HttpClient, Method};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use zeroize::Zeroizing;

use tauri::Emitter;
#[cfg(not(target_os = "android"))]
use tauri::Listener;
use tauri::Manager;

const fn str_to_bytes<const N: usize>(s: &str) -> [u8; N] {
    assert!(s.len() == N);
    let bytes = s.as_bytes();
    let mut arr = [0u8; N];
    let mut i = 0;
    while i < N {
        arr[i] = bytes[i];
        i += 1;
    }
    arr
}

// ARCHITECTURAL DECISION: Static Compile-Time Key (GrapheneOS Reliability)
// Hardcoded password instead of Device-Specific Random Key
// We explicitly bypass the OS Keyring/Keystore to prevent data loss.
// Hardware-backed keys are often invalidated by GrapheneOS updates or security state changes.
// This static key ensures the vault remains accessible across updates and Seedvault backups.
// Security relies on the strict Application Sandbox.
// REQUIREMENT: Value must be exactly 32 bytes.
const VAULT_PASSWORD: [u8; 32] = str_to_bytes(std::env!("VAULT_PASSWORD"));
const VAULT_FILE: &str = "openhome_vault.hold";
const CLIENT_NAME: &str = "openhome_client";
const API_KEY_STORE_KEY: &str = "api_key";

#[derive(Serialize, Clone, Copy)]
pub enum ApiKeyStatus {
    NotSet,
    Locked,
    Unlocked,
}

fn validate_base_url(url: &str) -> Result<String, AppError> {
    let url_str = url.trim_end_matches('/');
    let parsed_url =
        url::Url::parse(url_str).map_err(|_| AppError::Config("Invalid base URL".into()))?;
    if parsed_url.scheme() != "http" && parsed_url.scheme() != "https" {
        return Err(AppError::Config(
            "Base URL must use http or https scheme".into(),
        ));
    }
    Ok(url_str.to_string())
}

#[cfg(any(target_os = "android", target_os = "ios"))]
fn convert_biometric_error(err: tauri_plugin_biometric::Error) -> AppError {
    let err_str = err.to_string().to_lowercase();
    if err_str.contains("cancel")
        || err_str.contains("cancelled")
        || err_str.contains("user cancelled")
    {
        AppError::BiometricCancelled
    } else if err_str.contains("not available")
        || err_str.contains("unavailable")
        || err_str.contains("no biometric")
    {
        AppError::BiometricUnavailable(err.to_string())
    } else {
        AppError::BiometricFailed
    }
}

fn is_path_safe(path: &str) -> bool {
    let normalized = path.trim_start_matches('/');

    if normalized.is_empty() || normalized.contains("..") || normalized.contains('\\') {
        return false;
    }

    if normalized.contains("://")
        || normalized.starts_with("file:")
        || normalized.starts_with("data:")
    {
        return false;
    }

    if let Ok(decoded) = percent_encoding::percent_decode_str(normalized).decode_utf8()
        && (decoded.contains("..") || decoded.contains('\\'))
    {
        return false;
    }

    let path_obj = Path::new(normalized);
    if path_obj.components().any(|c| {
        if let Some(comp) = c.as_os_str().to_str() {
            comp == ".." || comp.contains('\0')
        } else {
            false
        }
    }) {
        return false;
    }

    true
}

// Define a struct to hold our configuration state
pub struct ConfigState {
    pub config: config::AppConfig,
    pub api_key: Mutex<Option<String>>,
    pub http_client: HttpClient,
    pub last_unlock_time: Mutex<Option<Instant>>,
}

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> Result<String, String> {
    let trimmed_name = name.trim();
    if trimmed_name.is_empty() {
        return Err("Name cannot be empty".to_string());
    }
    if trimmed_name.len() > 100 {
        return Err("Name is too long".to_string());
    }

    Ok(format!(
        "Hello, {}! You've been greeted from Rust!",
        trimmed_name
    ))
}

/// Returns the path to the vault file in app data directory
fn get_vault_path(app: &tauri::AppHandle) -> Result<PathBuf, AppError> {
    let app_data_dir = app
        .path()
        .app_local_data_dir()
        .map_err(|e| AppError::Config(format!("Failed to get app data dir: {}", e)))?;
    let vault_path = app_data_dir.join(VAULT_FILE);
    Ok(vault_path)
}

/// Creates a KeyProvider from the static vault password
fn create_key_provider() -> Result<KeyProvider, AppError> {
    let password_bytes = Zeroizing::new(VAULT_PASSWORD.to_vec());
    KeyProvider::try_from(password_bytes)
        .map_err(|e| AppError::VaultUnavailable(format!("Failed to create key provider: {:?}", e)))
}

/// Opens or creates the Stronghold vault at the given path
fn open_stronghold(vault_path: &Path) -> Result<Stronghold, AppError> {
    let stronghold = Stronghold::default();
    let snapshot_path = SnapshotPath::from_path(vault_path);
    let key_provider = create_key_provider()?;

    // If snapshot exists, load it
    let snapshot_exists = snapshot_path.exists();
    if snapshot_exists {
        match stronghold.load_snapshot(&key_provider, &snapshot_path) {
            Ok(_) => {}
            Err(e) => {
                return Err(AppError::VaultUnavailable(format!(
                    "Failed to load vault snapshot: {:?}",
                    e
                )));
            }
        }
    }

    Ok(stronghold)
}

/// Loads or creates the Stronghold client
fn load_stronghold_client(app: &tauri::AppHandle) -> Result<(Stronghold, Client), AppError> {
    let vault_path = get_vault_path(app)?;
    let stronghold = open_stronghold(&vault_path)?;

    let client_name_bytes = CLIENT_NAME.as_bytes().to_vec();

    // Try to load existing client, or create new one
    let client = match stronghold.load_client(client_name_bytes.clone()) {
        Ok(client) => client,
        Err(_) => stronghold.create_client(client_name_bytes).map_err(|e| {
            AppError::VaultUnavailable(format!("Failed to create Stronghold client: {:?}", e))
        })?,
    };

    Ok((stronghold, client))
}

/// Saves the Stronghold snapshot to disk
fn commit_stronghold(stronghold: &Stronghold, vault_path: &Path) -> Result<(), AppError> {
    let snapshot_path = SnapshotPath::from_path(vault_path);
    let key_provider = create_key_provider()?;
    let client_name_bytes = CLIENT_NAME.as_bytes().to_vec();

    // CRITICAL: Must write client before committing
    stronghold
        .write_client(client_name_bytes)
        .map_err(|e| AppError::VaultUnavailable(format!("Failed to write client: {:?}", e)))?;

    stronghold
        .commit_with_keyprovider(&snapshot_path, &key_provider)
        .map_err(|e| AppError::VaultUnavailable(format!("Failed to commit vault: {:?}", e)))
}

#[tauri::command]
#[allow(unused_variables)]
async fn set_api_key(
    app: tauri::AppHandle,
    state: tauri::State<'_, ConfigState>,
    key: String,
) -> Result<(), AppError> {
    let trimmed = key.trim();
    if trimmed.is_empty() {
        return Err(AppError::Config("API key cannot be empty".into()));
    }

    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        use tauri_plugin_biometric::{AuthOptions, BiometricExt};

        let options = AuthOptions {
            allow_device_credential: true,
            cancel_title: Some("API key will not be saved".to_string()),
            fallback_title: Some("Use device PIN/password".to_string()),
            title: Some("Save API Key".to_string()),
            subtitle: Some("Authenticate to securely save your API key".to_string()),
            confirmation_required: Some(true),
        };

        app.biometric()
            .authenticate("Save API Key".to_string(), options)
            .map_err(convert_biometric_error)?;
    }

    // Load stronghold and get client
    let (stronghold, client) = load_stronghold_client(&app).map_err(|e| {
        eprintln!("[set_api_key] Failed to load stronghold client: {:?}", e);
        e
    })?;

    let store = client.store();

    // Store API key in Stronghold
    let key_bytes = API_KEY_STORE_KEY.as_bytes().to_vec();
    let value_bytes = trimmed.as_bytes().to_vec();
    store.insert(key_bytes, value_bytes, None).map_err(|e| {
        eprintln!("[set_api_key] Failed to insert into store: {:?}", e);
        AppError::VaultUnavailable(format!("Failed to store API key: {:?}", e))
    })?;

    // Save stronghold to disk
    let vault_path = get_vault_path(&app).map_err(|e| {
        eprintln!("[set_api_key] Failed to get vault path: {:?}", e);
        e
    })?;
    commit_stronghold(&stronghold, &vault_path).map_err(|e| {
        eprintln!("[set_api_key] Failed to commit stronghold: {:?}", e);
        e
    })?;

    // Cache the API key in memory
    if let Ok(mut cache) = state.api_key.lock() {
        *cache = Some(trimmed.to_string());
    }

    if let Ok(mut last_time) = state.last_unlock_time.lock() {
        *last_time = Some(Instant::now());
    }

    Ok(())
}

#[tauri::command]
#[allow(unused_variables)]
async fn unlock_and_cache_api_key(
    app: tauri::AppHandle,
    state: tauri::State<'_, ConfigState>,
) -> Result<(), AppError> {
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        use tauri_plugin_biometric::{AuthOptions, BiometricExt};

        let options = AuthOptions {
            allow_device_credential: true,
            cancel_title: Some("API key will remain locked".to_string()),
            fallback_title: Some("Use device PIN/password".to_string()),
            title: Some("Unlock API Key".to_string()),
            subtitle: Some("Authenticate to access your API key".to_string()),
            confirmation_required: Some(true),
        };

        app.biometric()
            .authenticate("Unlock API Key".to_string(), options)
            .map_err(convert_biometric_error)?;
    }

    // Load API key from Stronghold
    let (stronghold, client) = load_stronghold_client(&app)?;
    let _ = stronghold; // Keep stronghold alive while we use client
    let store = client.store();

    let key_bytes = API_KEY_STORE_KEY.as_bytes().to_vec();
    let value_bytes = store
        .get(&key_bytes)
        .map_err(|_| AppError::MissingApiKey)?
        .ok_or(AppError::MissingApiKey)?;

    let api_key = String::from_utf8(value_bytes).map_err(|_| AppError::MissingApiKey)?;

    // Cache the API key in memory
    if let Ok(mut cache) = state.api_key.lock() {
        *cache = Some(api_key);
    }

    if let Ok(mut last_time) = state.last_unlock_time.lock() {
        *last_time = Some(Instant::now());
    }

    Ok(())
}

#[tauri::command]
fn clear_api_key_cache(state: tauri::State<'_, ConfigState>) -> Result<(), AppError> {
    if let Ok(mut cache) = state.api_key.lock() {
        *cache = None;
    }
    Ok(())
}

#[tauri::command]
#[allow(unused_variables)]
async fn biometric_resume_auth(
    app: tauri::AppHandle,
    state: tauri::State<'_, ConfigState>,
    timeout_minutes: u64,
) -> Result<(), AppError> {
    let timeout_duration = Duration::from_secs(timeout_minutes * 60);

    if let Ok(last_time) = state.last_unlock_time.lock()
        && let Some(unlock_time) = *last_time
    {
        let elapsed = unlock_time.elapsed();
        if elapsed < timeout_duration {
            return Ok(());
        }
    }

    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        use tauri_plugin_biometric::{AuthOptions, BiometricExt};

        let options = AuthOptions {
            allow_device_credential: true,
            cancel_title: Some("API key will remain locked".to_string()),
            fallback_title: Some("Use device PIN/password".to_string()),
            title: Some("Unlock API Key".to_string()),
            subtitle: Some("Authenticate to access your API key".to_string()),
            confirmation_required: Some(true),
        };

        app.biometric()
            .authenticate("Unlock API Key".to_string(), options)
            .map_err(convert_biometric_error)?;
    }

    // Load API key from Stronghold
    let (stronghold, client) = load_stronghold_client(&app)?;
    let _ = stronghold; // Keep stronghold alive while we use client
    let store = client.store();

    let key_bytes = API_KEY_STORE_KEY.as_bytes().to_vec();
    let value_bytes = store
        .get(&key_bytes)
        .map_err(|_| AppError::MissingApiKey)?
        .ok_or(AppError::MissingApiKey)?;

    let api_key = String::from_utf8(value_bytes).map_err(|_| AppError::MissingApiKey)?;

    // Cache the API key in memory
    if let Ok(mut cache) = state.api_key.lock() {
        *cache = Some(api_key);
    }

    if let Ok(mut last_time) = state.last_unlock_time.lock() {
        *last_time = Some(Instant::now());
    }

    Ok(())
}

#[tauri::command]
#[allow(unused_variables)]
async fn reset_api_key(
    app: tauri::AppHandle,
    state: tauri::State<'_, ConfigState>,
) -> Result<(), AppError> {
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        use tauri_plugin_biometric::{AuthOptions, BiometricExt};

        let options = AuthOptions {
            allow_device_credential: true,
            cancel_title: Some("Reset cancelled".to_string()),
            fallback_title: Some("Use device PIN/password".to_string()),
            title: Some("Reset API Key".to_string()),
            subtitle: Some("Authenticate to reset your API key".to_string()),
            confirmation_required: Some(true),
        };

        app.biometric()
            .authenticate("Reset API Key".to_string(), options)
            .map_err(convert_biometric_error)?;
    }

    // Clear cache
    if let Ok(mut cache) = state.api_key.lock() {
        *cache = None;
    }

    // Remove from Stronghold
    let (stronghold, client) = load_stronghold_client(&app)?;
    let key_bytes = API_KEY_STORE_KEY.as_bytes().to_vec();

    // Create location for the key and revoke it
    let location = Location::generic(
        API_KEY_STORE_KEY.as_bytes().to_vec(),
        API_KEY_STORE_KEY.as_bytes().to_vec(),
    );
    client
        .revoke_data(&location)
        .map_err(|e| AppError::VaultUnavailable(format!("Failed to remove API key: {:?}", e)))?;

    // Also try to remove from store
    let store = client.store();
    let _ = store.delete(&key_bytes);

    // Save stronghold to disk
    let vault_path = get_vault_path(&app)?;
    commit_stronghold(&stronghold, &vault_path)?;

    if let Ok(mut last_time) = state.last_unlock_time.lock() {
        *last_time = None;
    }

    Ok(())
}

#[tauri::command]
async fn get_api_key_status(
    app: tauri::AppHandle,
    state: tauri::State<'_, ConfigState>,
) -> Result<ApiKeyStatus, AppError> {
    // Check if already unlocked in cache
    if let Ok(cache) = state.api_key.lock()
        && cache.is_some()
    {
        return Ok(ApiKeyStatus::Unlocked);
    }

    // Check if vault exists and has API key
    let vault_path = match get_vault_path(&app) {
        Ok(path) => path,
        Err(_) => return Ok(ApiKeyStatus::NotSet),
    };

    if !vault_path.exists() {
        return Ok(ApiKeyStatus::NotSet);
    }

    match load_stronghold_client(&app) {
        Ok((stronghold, client)) => {
            let _ = stronghold; // Keep stronghold alive
            let store = client.store();
            let key_bytes = API_KEY_STORE_KEY.as_bytes().to_vec();
            match store.get(&key_bytes) {
                Ok(Some(_)) => Ok(ApiKeyStatus::Locked),
                _ => Ok(ApiKeyStatus::NotSet),
            }
        }
        Err(_) => Ok(ApiKeyStatus::NotSet),
    }
}

#[tauri::command]
fn get_api_config(state: tauri::State<ConfigState>) -> Result<config::ApiConfig, String> {
    state.config.api.sanitize()
}

#[derive(Serialize)]
struct KeyringDiagnostics {
    key_present: bool,
    key_length: Option<usize>,
    keyring_accessible: bool,
    error_message: Option<String>,
}

#[tauri::command]
async fn get_keyring_diagnostics(app: tauri::AppHandle) -> Result<KeyringDiagnostics, AppError> {
    let vault_path = match get_vault_path(&app) {
        Ok(path) => path,
        Err(e) => {
            return Ok(KeyringDiagnostics {
                key_present: false,
                key_length: None,
                keyring_accessible: false,
                error_message: Some(e.to_string()),
            });
        }
    };

    if !vault_path.exists() {
        return Ok(KeyringDiagnostics {
            key_present: false,
            key_length: None,
            keyring_accessible: true,
            error_message: Some("Vault file does not exist".to_string()),
        });
    }

    match load_stronghold_client(&app) {
        Ok((stronghold, client)) => {
            let _ = stronghold; // Keep stronghold alive
            let store = client.store();
            let key_bytes = API_KEY_STORE_KEY.as_bytes().to_vec();
            match store.get(&key_bytes) {
                Ok(Some(value_bytes)) => Ok(KeyringDiagnostics {
                    key_present: true,
                    key_length: Some(value_bytes.len()),
                    keyring_accessible: true,
                    error_message: None,
                }),
                _ => Ok(KeyringDiagnostics {
                    key_present: false,
                    key_length: None,
                    keyring_accessible: true,
                    error_message: Some("No API key entry found".to_string()),
                }),
            }
        }
        Err(e) => Ok(KeyringDiagnostics {
            key_present: false,
            key_length: None,
            keyring_accessible: false,
            error_message: Some(e.to_string()),
        }),
    }
}

#[derive(Serialize)]
struct ApiResponse {
    status: u16,
    data: serde_json::Value,
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
#[allow(unused_variables)]
async fn call_api(
    _app: tauri::AppHandle,
    state: tauri::State<'_, ConfigState>,
    path: String,
    method: String,
    body: Option<serde_json::Value>,
    base_url: String,
    timeout_seconds: u64,
    api_key_override: Option<String>,
) -> Result<ApiResponse, AppError> {
    let method_upper = method.to_uppercase();
    let req_method = match method_upper.as_str() {
        "GET" => Method::GET,
        "POST" => Method::POST,
        "DELETE" => Method::DELETE,
        _ => {
            return Err(AppError::Config(
                "Only GET, POST, and DELETE are allowed".into(),
            ));
        }
    };

    let path_normalized = path.trim_start_matches('/');
    if !is_path_safe(path_normalized) {
        return Err(AppError::Config("Invalid path".into()));
    }
    if !path_normalized.starts_with("api/") {
        return Err(AppError::Config("Path must start with /api/".into()));
    }

    if !(1..=300).contains(&timeout_seconds) {
        return Err(AppError::Config(
            "Timeout must be between 1 and 300 seconds".into(),
        ));
    }

    if base_url.trim().is_empty() {
        return Err(AppError::Config("Base URL cannot be empty".into()));
    }
    let base_url = validate_base_url(&base_url)?;

    let url = format!("{}/{}", base_url, path_normalized);

    let key: Option<String> = if let Some(ref api_key) = api_key_override {
        let trimmed = api_key.trim();
        if !trimmed.is_empty() {
            let has_cached = state
                .api_key
                .lock()
                .map(|cache| cache.is_some())
                .unwrap_or(false);

            if !has_cached {
                Some(trimmed.to_string())
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    let key = if key.is_some() {
        key
    } else {
        let cached_key = state
            .api_key
            .lock()
            .map_err(|_| AppError::Config("Failed to access API key cache".into()))?
            .clone();

        Some(cached_key.ok_or(AppError::MissingApiKey)?)
    };

    let mut request = state
        .http_client
        .request(req_method, url)
        .timeout(Duration::from_secs(timeout_seconds));

    if let Some(k) = key {
        request = request.bearer_auth(k);
    }

    if let Some(b) = body {
        request = request.json(&b);
    }

    let response = request.send().await?;
    let status = response.status().as_u16();
    let text = response.text().await?;

    let data: serde_json::Value = if text.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_str(&text).unwrap_or(serde_json::Value::String(text))
    };

    // Map 401 to structured error
    if status == 401 {
        return Err(AppError::ApiKeyRejected);
    }

    let result = ApiResponse { status, data };

    Ok(result)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(target_os = "android")]
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Debug)
            .with_tag("openhome"),
    );

    let config = match config::AppConfig::load() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Fatal error loading config: {}", e);
            std::process::exit(1);
        }
    };

    if let Err(_error) = config.api.clone().sanitize() {
        eprintln!("API config validation warning: {_error}");
    }

    let http_client = HttpClient::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .expect("Failed to create HTTP client");

    let builder = tauri::Builder::default();

    #[cfg(any(target_os = "android", target_os = "ios"))]
    let builder = builder.plugin(tauri_plugin_biometric::init());

    if let Err(e) = builder
        .manage(ConfigState {
            config,
            api_key: Mutex::new(None),
            http_client,
            last_unlock_time: Mutex::new(None),
        })
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_http::init())
        .setup(|app| {
            let handle = app.handle().clone();

            // Handle app resume events (works for iOS, macOS, Linux, Windows)
            #[cfg(not(target_os = "android"))]
            {
                let handle_emit = handle.clone();
                handle.listen("tauri://resumed", move |_| {
                    let _ = handle_emit.emit_to("main", "auth:resume", ());
                });
            }

            // Android resume handling - use window focus events
            #[cfg(target_os = "android")]
            {
                if let Some(window) = app.get_webview_window("main") {
                    let handle_clone = handle.clone();
                    window.on_window_event(move |event| {
                        if let tauri::WindowEvent::Focused(focused) = event {
                            if *focused {
                                let _ = handle_clone.emit("auth:resume", ());
                            }
                        }
                    });
                }
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            get_api_config,
            call_api,
            get_keyring_diagnostics,
            set_api_key,
            unlock_and_cache_api_key,
            clear_api_key_cache,
            get_api_key_status,
            biometric_resume_auth,
            reset_api_key,
        ])
        .run(tauri::generate_context!())
    {
        eprintln!("error running tauri application: {}", e);
        #[cfg(not(mobile))]
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_path_safe_valid_paths() {
        assert!(is_path_safe("api/timeline"));
        assert!(is_path_safe("api/feed/items"));
        assert!(is_path_safe("api/v1/users/123"));
    }

    #[test]
    fn test_is_path_safe_invalid_paths() {
        assert!(!is_path_safe("../etc/passwd"));
        assert!(!is_path_safe("api/../../secret"));
        assert!(!is_path_safe("api\\windows\\path"));
        assert!(!is_path_safe("file:///etc/passwd"));
        assert!(!is_path_safe("data:text/plain,malicious"));
        assert!(!is_path_safe("api/%2e%2e/etc"));
        assert!(!is_path_safe("api/.."));
    }

    #[test]
    fn test_validate_base_url_valid() {
        assert!(validate_base_url("http://localhost:8080").is_ok());
        assert!(validate_base_url("https://api.example.com").is_ok());
        assert!(validate_base_url("http://192.168.1.1:3000").is_ok());
    }

    #[test]
    fn test_validate_base_url_invalid() {
        assert!(validate_base_url("ftp://example.com").is_err());
        assert!(validate_base_url("file:///path").is_err());
        assert!(validate_base_url("invalid-url").is_err());
    }
}
