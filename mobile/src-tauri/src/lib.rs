pub mod config;

// Define a struct to hold our configuration state
pub struct ConfigState(pub config::AppConfig);

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
    state.0.api.sanitize()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
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
        .manage(ConfigState(config))
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_http::init())
        .invoke_handler(tauri::generate_handler![greet, get_api_config])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
