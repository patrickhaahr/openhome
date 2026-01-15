use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ApiConfig {
    pub base_url: String,
    pub timeout_seconds: u64,
}

impl ApiConfig {
    pub fn sanitize(&self) -> Result<Self, String> {
        if self.base_url.trim().is_empty() {
            return Err("API base URL cannot be empty".to_string());
        }
        if self.timeout_seconds == 0 {
            return Err("Timeout seconds must be greater than zero".to_string());
        }

        Ok(self.clone())
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AppConfig {
    pub api: ApiConfig,
}

impl AppConfig {
    pub fn load() -> Result<Self, String> {
        #[cfg(debug_assertions)]
        let config_str = include_str!("../config.dev.toml");

        #[cfg(not(debug_assertions))]
        let config_str = include_str!("../config.prod.toml");

        toml::from_str(config_str).map_err(|e| format!("Failed to parse config: {}", e))
    }
}
