use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize)]
pub struct ContainerListResponse {
    pub containers: Vec<ContainerStatus>,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerStatus {
    pub name: String,
    #[serde(rename = "status")]
    pub display_status: String,
    pub state: String,
    #[serde(rename = "HealthStatus")]
    pub health_status: Option<String>,
    pub uptime_seconds: Option<i64>,
    pub image: String,
    pub ports: Vec<String>,
    pub labels: HashMap<String, String>,
    #[serde(rename = "Created")]
    pub created_at: String,
    pub restart_count: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ContainerDetailResponse {
    pub name: String,
    #[serde(rename = "status")]
    pub display_status: String,
    pub state: String,
    pub health_status: Option<String>,
    pub uptime_seconds: Option<i64>,
    pub image: String,
    pub image_id: String,
    pub ports: Vec<String>,
    pub volumes: Vec<String>,
    pub networks: Vec<String>,
    pub labels: HashMap<String, String>,
    #[serde(rename = "Created")]
    pub created_at: String,
    #[serde(rename = "State")]
    pub started_at: String,
    pub restart_count: i32,
    pub memory_usage_mb: Option<f64>,
    pub cpu_percent: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RestartResponse {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct RestartRequest {
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,
}

fn default_timeout() -> u64 {
    10
}
