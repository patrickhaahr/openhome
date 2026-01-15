pub mod config;
pub mod error;

use error::AppError;
use reqwest::Method;
use serde::Serialize;
use std::time::Duration;
use std::sync::Mutex;
use keyring::Entry;

// Define a struct to hold our configuration state
pub struct ConfigState {
    pub config: config::AppConfig,
    pub api_key: Mutex<Option<String>>,
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
fn get_api_config(state: tauri::State<ConfigState>) -> Result<config::ApiConfig, String> {
    state.config.api.sanitize()
}

#[tauri::command]
async fn save_api_key(
    state: tauri::State<'_, ConfigState>, 
    key: String
) -> Result<(), AppError> {
    let trimmed = key.trim();
    if trimmed.is_empty() {
        return Err(AppError::Config("API key cannot be empty".into()));
    }
    
    // Save to keyring
    let entry = Entry::new("com.patrickhaahr.home-app", "api_key")?;
    entry.set_password(trimmed)?;

    // Update memory cache
    if let Ok(mut cache) = state.api_key.lock() {
        *cache = Some(trimmed.to_string());
    }

    Ok(())
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
    let entry = match Entry::new("com.patrickhaahr.home-app", "api_key") {
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

#[tauri::command]
async fn call_api(
    state: tauri::State<'_, ConfigState>,
    path: String,
    method: String,
    body: Option<serde_json::Value>,
    base_url: String,
    timeout_seconds: u64,
    #[allow(unused_variables)] api_key_override: Option<String>,
) -> Result<ApiResponse, AppError> {
    let method_upper = method.to_uppercase();
    let req_method = match method_upper.as_str() {
        "GET" => Method::GET,
        "POST" => Method::POST,
        _ => return Err(AppError::Config("Only GET and POST are allowed".into())),
    };

    if base_url.trim().is_empty() {
        return Err(AppError::Config("Base URL cannot be empty".into()));
    }
    if !(1..=300).contains(&timeout_seconds) {
        return Err(AppError::Config(
            "Timeout must be between 1 and 300 seconds".into(),
        ));
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(timeout_seconds))
        .build()?;

    let url = format!(
        "{}/{}",
        base_url.trim_end_matches('/'),
        path.trim_start_matches('/')
    );

    // Key resolution: debug override (debug only) -> memory cache -> keyring
    #[cfg(debug_assertions)]
    let key: Option<String> = if let Some(ref override_key) = api_key_override {
        let trimmed = override_key.trim();
        if !trimmed.is_empty() {
            Some(trimmed.to_string())
        } else {
            None
        }
    } else {
        None
    };

    #[cfg(not(debug_assertions))]
    let key: Option<String> = None;

    // If no override, try cache then keyring
    let key = if key.is_some() {
        #[cfg(debug_assertions)]
        eprintln!("[call_api] Using override key");
        key
    } else {
        // Try memory cache first
        let cached_key = if let Ok(cache) = state.api_key.lock() {
            cache.clone()
        } else {
            None
        };

        if let Some(k) = cached_key {
            #[cfg(debug_assertions)]
            eprintln!("[call_api] Using cached key");
            Some(k)
        } else {
            // Try keyring
            let entry = Entry::new("com.patrickhaahr.home-app", "api_key")?;
            match entry.get_password() {
                Ok(k) => {
                    #[cfg(debug_assertions)]
                    eprintln!("[call_api] Got key from keyring, length={}", k.len());
                    
                    // Update cache
                    if let Ok(mut cache) = state.api_key.lock() {
                        *cache = Some(k.clone());
                    }
                    
                    Some(k)
                }
                Err(keyring::Error::NoEntry) => {
                    #[cfg(debug_assertions)]
                    eprintln!("[call_api] No key in keyring");
                    None
                }
                Err(e) => {
                    #[cfg(debug_assertions)]
                    eprintln!("[call_api] Keyring error: {}", e);
                    // Don't fail the call, just no auth
                    None
                }
            }
        }
    };

    let mut request = client.request(req_method, url);

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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(target_os = "android")]
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Debug)
            .with_tag("home-app")
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

    tauri::Builder::default()
        .manage(ConfigState {
            config,
            api_key: Mutex::new(None),
        })
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_http::init())
        .invoke_handler(tauri::generate_handler![
            greet,
            get_api_config,
            save_api_key,
            call_api,
            get_keyring_diagnostics
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
