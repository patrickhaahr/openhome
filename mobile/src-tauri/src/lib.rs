pub mod config;
pub mod crypto;
pub mod error;

use base64::Engine;
use error::AppError;
use keyring::Entry;
use reqwest::{Client, Method};
use serde::Serialize;
use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

#[cfg(any(target_os = "android", target_os = "ios"))]
use tauri_plugin_biometric::BiometricExt;

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
    pub master_key: Mutex<Option<[u8; 32]>>,
    pub http_client: Client,
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

#[tauri::command]
async fn set_api_key(
    _app: tauri::AppHandle,
    state: tauri::State<'_, ConfigState>,
    key: String,
) -> Result<(), AppError> {
    let trimmed = key.trim();
    if trimmed.is_empty() {
        return Err(AppError::Config("API key cannot be empty".into()));
    }

    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        use tauri_plugin_biometric::{BiometricExt, AuthOptions};

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
            .map_err(|e| AppError::BiometricUnavailable(e.to_string()))?;
    }

    let master_key = state.master_key
        .lock()
        .map_err(|_| AppError::Config("Failed to access master key".into()))?
        .ok_or(AppError::Config("Master key not initialized".into()))?;

    let payload = crypto::encrypt_api_key(trimmed, &master_key)?;

    let encrypted_json = serde_json::to_string(&payload)?;
    let entry = Entry::new("com.patrickhaahr.openhome", "api_key_encrypted")?;
    entry.set_password(&encrypted_json)?;

    if let Ok(mut cache) = state.api_key.lock() {
        *cache = Some(trimmed.to_string());
    }

    Ok(())
}

#[tauri::command]
async fn unlock_and_cache_api_key(
    _app: tauri::AppHandle,
    state: tauri::State<'_, ConfigState>,
) -> Result<(), AppError> {
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        use tauri_plugin_biometric::{BiometricExt, AuthOptions};

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
            .map_err(|e| AppError::BiometricUnavailable(e.to_string()))?;
    }

    let entry = Entry::new("com.patrickhaahr.openhome", "api_key_encrypted")?;
    let encrypted_json = entry.get_password()
        .map_err(|e| match e {
            keyring::Error::NoEntry => AppError::MissingApiKey,
            _ => AppError::KeyringUnavailable(e.to_string()),
        })?;

    let payload: crypto::EncryptedPayload = serde_json::from_str(&encrypted_json)
        .map_err(|e| AppError::Config(format!("Invalid encrypted payload: {}", e)))?;

    let master_key = state.master_key
        .lock()
        .map_err(|_| AppError::Config("Failed to access master key".into()))?
        .ok_or(AppError::Config("Master key not initialized".into()))?;

    let api_key = crypto::decrypt_api_key(&payload, &master_key)?;

    if let Ok(mut cache) = state.api_key.lock() {
        *cache = Some(api_key);
    }

    Ok(())
}

#[tauri::command]
fn clear_api_key_cache(state: tauri::State<'_, ConfigState>) -> Result<(), AppError> {
    if let Ok(mut cache) = state.api_key.lock()
        && let Some(mut key) = cache.take()
    {
        crypto::zeroize_string(&mut key);
    }
    Ok(())
}

#[tauri::command]
fn get_api_key_status(state: tauri::State<'_, ConfigState>) -> ApiKeyStatus {
    if let Ok(cache) = state.api_key.lock()
        && cache.is_some()
    {
        return ApiKeyStatus::Unlocked;
    }

    let entry = match Entry::new("com.patrickhaahr.openhome", "api_key_encrypted") {
        Ok(e) => e,
        Err(_) => return ApiKeyStatus::NotSet,
    };

    match entry.get_password() {
        Ok(_) => ApiKeyStatus::Locked,
        Err(keyring::Error::NoEntry) => ApiKeyStatus::NotSet,
        Err(_) => ApiKeyStatus::NotSet,
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
async fn get_keyring_diagnostics() -> Result<KeyringDiagnostics, AppError> {
    let entry = match Entry::new("com.patrickhaahr.openhome", "api_key") {
        Ok(e) => e,
        Err(e) => {
            return Ok(KeyringDiagnostics {
                key_present: false,
                key_length: None,
                keyring_accessible: false,
                error_message: Some(e.to_string()),
            });
        }
    };

    match entry.get_password() {
        Ok(password) => Ok(KeyringDiagnostics {
            key_present: true,
            key_length: Some(password.len()),
            keyring_accessible: true,
            error_message: None,
        }),
        Err(keyring::Error::NoEntry) => Ok(KeyringDiagnostics {
            key_present: false,
            key_length: None,
            keyring_accessible: true,
            error_message: Some("No Entry".to_string()),
        }),
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

    let base_url = if cfg!(debug_assertions) {
        if base_url.trim().is_empty() {
            return Err(AppError::Config("Base URL cannot be empty".into()));
        }
        validate_base_url(&base_url)?
    } else {
        validate_base_url(&state.config.api.base_url)?
    };

    let url = format!("{}/{}", base_url, path_normalized);

    let key: Option<String> = if cfg!(debug_assertions) {
        if let Some(ref override_key) = api_key_override {
            let trimmed = override_key.trim();
            if !trimmed.is_empty() {
                #[cfg(any(target_os = "android", target_os = "ios"))]
                {
                    use tauri_plugin_biometric::{BiometricExt, AuthOptions};
                    let options = AuthOptions {
                        allow_device_credential: true,
                        cancel_title: Some("API call will fail".to_string()),
                        fallback_title: Some("Use device PIN/password".to_string()),
                        title: Some("API Key Override".to_string()),
                        subtitle: Some("Authenticate to use debug override".to_string()),
                        confirmation_required: Some(true),
                    };
                    app.biometric()
                        .authenticate("API Key Override".to_string(), options)
                        .map_err(|e| AppError::BiometricUnavailable(e.to_string()))?;
                }
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
        let cached_key = state.api_key
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

    Ok(ApiResponse { status, data })
}

fn initialize_or_create_master_key() -> [u8; 32] {
    let entry = match Entry::new("com.patrickhaahr.openhome", "master_key") {
        Ok(e) => e,
        Err(e) => {
            #[cfg(debug_assertions)]
            eprintln!("[master_key] Keyring init error: {}", e);

            return crypto::generate_master_key().unwrap_or([0u8; 32]);
        }
    };

    match entry.get_password() {
        Ok(stored_key) => {
            let key_bytes = base64::engine::general_purpose::STANDARD.decode(&stored_key);
            match key_bytes {
                Ok(ref bytes) if bytes.len() == 32 => {
                    let mut key: [u8; 32] = [0u8; 32];
                    key.copy_from_slice(bytes);
                    #[cfg(debug_assertions)]
                    eprintln!("[master_key] Loaded from keyring");
                    key
                }
                _ => {
                    #[cfg(debug_assertions)]
                    eprintln!("[master_key] Invalid stored key, regenerating");
                    create_and_store_master_key(&entry)
                }
            }
        }
        Err(keyring::Error::NoEntry) => {
            #[cfg(debug_assertions)]
            eprintln!("[master_key] No entry, creating new key");
            create_and_store_master_key(&entry)
        }
        Err(e) => {
            #[cfg(debug_assertions)]
            eprintln!("[master_key] Keyring error: {}, regenerating", e);
            create_and_store_master_key(&entry)
        }
    }
}

fn create_and_store_master_key(entry: &keyring::Entry) -> [u8; 32] {
    let key = crypto::generate_master_key().unwrap_or([0u8; 32]);
    let encoded = base64::engine::general_purpose::STANDARD.encode(key);

    if let Err(e) = entry.set_password(&encoded) {
        #[cfg(debug_assertions)]
        eprintln!("[master_key] Failed to store: {}", e);
    }

    key
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

    if let Err(error) = config.api.clone().sanitize() {
        #[cfg(debug_assertions)]
        eprintln!("API config validation warning: {error}");
    }

    let master_key = initialize_or_create_master_key();

    let http_client = reqwest::Client::builder()
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
            master_key: Mutex::new(Some(master_key)),
            http_client,
        })
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_http::init())
        .invoke_handler(tauri::generate_handler![
            greet,
            get_api_config,
            call_api,
            get_keyring_diagnostics,
            set_api_key,
            unlock_and_cache_api_key,
            clear_api_key_cache,
            get_api_key_status,
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
