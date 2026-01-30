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

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            api: ApiConfig {
                base_url: String::new(),
                timeout_seconds: 30,
            },
        }
    }
}

impl AppConfig {
    pub fn load() -> Result<Self, String> {
        Ok(Self::default())
    }
}
