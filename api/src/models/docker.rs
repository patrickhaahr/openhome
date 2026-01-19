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

#[derive(Debug, Serialize, Deserialize)]
pub struct StartResponse {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StopResponse {
    pub success: bool,
    pub message: String,
    pub stopped: bool,
}

#[derive(Debug, Deserialize)]
pub struct StopRequest {
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_start_response_serialization() {
        let response = StartResponse {
            success: true,
            message: "Container started".to_string(),
        };
        
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"success\":true"));
        assert!(json.contains("\"message\":\"Container started\""));
    }

    #[test]
    fn test_start_response_deserialization() {
        let json = r#"{"success": true, "message": "Container started"}"#;
        let response: StartResponse = serde_json::from_str(json).unwrap();
        
        assert_eq!(response.success, true);
        assert_eq!(response.message, "Container started");
    }

    #[test]
    fn test_stop_response_serialization() {
        let response = StopResponse {
            success: true,
            message: "Container stopped".to_string(),
            stopped: true,
        };
        
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"success\":true"));
        assert!(json.contains("\"message\":\"Container stopped\""));
        assert!(json.contains("\"stopped\":true"));
    }

    #[test]
    fn test_stop_response_deserialization() {
        let json = r#"{"success": true, "message": "Container stopped", "stopped": true}"#;
        let response: StopResponse = serde_json::from_str(json).unwrap();
        
        assert_eq!(response.success, true);
        assert_eq!(response.message, "Container stopped");
        assert_eq!(response.stopped, true);
    }

    #[test]
    fn test_stop_response_with_stopped_false() {
        let json = r#"{"success": true, "message": "Container was not running", "stopped": false}"#;
        let response: StopResponse = serde_json::from_str(json).unwrap();
        
        assert_eq!(response.success, true);
        assert_eq!(response.stopped, false);
    }

    #[test]
    fn test_stop_request_deserialization_with_timeout() {
        let json = r#"{"timeout_seconds": 30}"#;
        let request: StopRequest = serde_json::from_str(json).unwrap();
        
        assert_eq!(request.timeout_seconds, 30);
    }

    #[test]
    fn test_stop_request_default_timeout() {
        let json = r#"{}"#;
        let request: StopRequest = serde_json::from_str(json).unwrap();
        
        assert_eq!(request.timeout_seconds, 10); // default_timeout() returns 10
    }

    #[test]
    fn test_stop_request_deserialization_without_timeout_field() {
        let json = r#""#;
        let result = serde_json::from_str::<StopRequest>(json);
        assert!(result.is_err());
    }
}
