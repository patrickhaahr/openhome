use axum::{
    Json, Router,
    extract::State,
    routing::{get, post},
};
use serde::Deserialize;

use crate::error::{AppError, Result};
use crate::services::adguard::AdguardStatusResponse;

#[derive(Debug, Deserialize)]
pub struct PauseRequest {
    pub minutes: u64,
}

pub fn router() -> Router<crate::AppState> {
    Router::new()
        .route("/api/adguard/status", get(get_status))
        .route("/api/adguard/enable", post(enable_protection))
        .route("/api/adguard/disable", post(disable_protection))
        .route("/api/adguard/pause", post(pause_protection))
}

async fn get_status(State(state): State<crate::AppState>) -> Result<Json<AdguardStatusResponse>> {
    let service = state
        .adguard_service
        .as_ref()
        .ok_or_else(|| AppError::ServiceUnavailable("AdGuard is not configured".to_string()))?;
    let status = service
        .get_status()
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to get AdGuard status: {}", e)))?;
    Ok(Json(status))
}

async fn enable_protection(
    State(state): State<crate::AppState>,
) -> Result<Json<AdguardStatusResponse>> {
    let service = state
        .adguard_service
        .as_ref()
        .ok_or_else(|| AppError::ServiceUnavailable("AdGuard is not configured".to_string()))?;
    let status = service
        .set_protection(true, None)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to enable protection: {}", e)))?;
    Ok(Json(status))
}

async fn disable_protection(
    State(state): State<crate::AppState>,
) -> Result<Json<AdguardStatusResponse>> {
    let service = state
        .adguard_service
        .as_ref()
        .ok_or_else(|| AppError::ServiceUnavailable("AdGuard is not configured".to_string()))?;
    let status = service
        .set_protection(false, None)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to disable protection: {}", e)))?;
    Ok(Json(status))
}

async fn pause_protection(
    State(state): State<crate::AppState>,
    Json(payload): Json<PauseRequest>,
) -> Result<Json<AdguardStatusResponse>> {
    const MAX_MINUTES: u64 = 1440;
    if payload.minutes == 0 || payload.minutes > MAX_MINUTES {
        return Err(AppError::Validation(format!(
            "minutes must be between 1 and {MAX_MINUTES}"
        )));
    }
    let service = state
        .adguard_service
        .as_ref()
        .ok_or_else(|| AppError::ServiceUnavailable("AdGuard is not configured".to_string()))?;
    let duration_ms = payload
        .minutes
        .checked_mul(60_000)
        .ok_or_else(|| AppError::Validation("minutes too large".to_string()))?;
    let status = service
        .set_protection(false, Some(duration_ms))
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to pause protection: {}", e)))?;
    Ok(Json(status))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::adguard::AdguardService;
    use serde_json::json;
    use sqlx::SqlitePool;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn create_mock_state(service: Option<AdguardService>) -> crate::AppState {
        crate::AppState {
            db: SqlitePool::connect_lazy("sqlite::memory:").unwrap(),
            adguard_service: service,
            docker_service: None,
            docker_cache: std::sync::Arc::new(tokio::sync::Mutex::new(
                crate::DockerCache::default(),
            )),
        }
    }

    #[tokio::test]
    async fn test_get_status_returns_503_when_service_not_configured() {
        let state = create_mock_state(None);

        let result = get_status(State(state)).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(format!("{}", err), "Service unavailable");
    }

    #[tokio::test]
    async fn test_enable_protection_returns_503_when_service_not_configured() {
        let state = create_mock_state(None);

        let result = enable_protection(State(state)).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(format!("{}", err), "Service unavailable");
    }

    #[tokio::test]
    async fn test_disable_protection_returns_503_when_service_not_configured() {
        let state = create_mock_state(None);

        let result = disable_protection(State(state)).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(format!("{}", err), "Service unavailable");
    }

    #[tokio::test]
    async fn test_pause_protection_returns_503_when_service_not_configured() {
        let state = create_mock_state(None);

        let payload = PauseRequest { minutes: 5 };
        let result = pause_protection(State(state), Json(payload)).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(format!("{}", err), "Service unavailable");
    }

    #[tokio::test]
    async fn test_get_status_with_mock_server() {
        let mock_server = MockServer::start().await;

        let raw_response = json!({
            "version": "v1.5.0",
            "language": "en",
            "dns_addresses": ["8.8.8.8"],
            "dns_port": 53,
            "http_port": 80,
            "protection_disabled_duration": 0,
            "protection_enabled": true,
            "protection_disabled_until": null,
            "dhcp_available": true,
            "running": true
        });

        Mock::given(method("GET"))
            .and(path("/control/status"))
            .and(header("Authorization", "Basic dGVzdDp0ZXN0"))
            .respond_with(ResponseTemplate::new(200).set_body_json(raw_response))
            .mount(&mock_server)
            .await;

        let service = AdguardService::new(&mock_server.uri(), "test", "test", false).unwrap();
        let state = create_mock_state(Some(service));

        let response = get_status(State(state)).await.unwrap();
        assert!(response.protection_enabled);
        assert_eq!(response.version, "v1.5.0");
        assert!(response.running);
    }

    #[tokio::test]
    async fn test_enable_protection_with_mock_server() {
        let mock_server = MockServer::start().await;

        let status_response = json!({
            "version": "v1.5.0",
            "language": "en",
            "dns_addresses": [],
            "dns_port": 53,
            "http_port": 80,
            "protection_disabled_duration": 0,
            "protection_enabled": true,
            "protection_disabled_until": null,
            "dhcp_available": false,
            "running": true
        });

        // Mock both the POST and the subsequent GET
        Mock::given(method("POST"))
            .and(path("/control/protection"))
            .and(header("Authorization", "Basic dGVzdDp0ZXN0"))
            .respond_with(ResponseTemplate::new(200).set_body_json(status_response.clone()))
            .mount(&mock_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/control/status"))
            .and(header("Authorization", "Basic dGVzdDp0ZXN0"))
            .respond_with(ResponseTemplate::new(200).set_body_json(status_response))
            .mount(&mock_server)
            .await;

        let service = AdguardService::new(&mock_server.uri(), "test", "test", false).unwrap();
        let state = create_mock_state(Some(service));

        let response = enable_protection(State(state)).await.unwrap();
        assert!(response.protection_enabled);
    }

    #[tokio::test]
    async fn test_disable_protection_with_mock_server() {
        let mock_server = MockServer::start().await;

        let status_response = json!({
            "version": "v1.5.0",
            "language": "en",
            "dns_addresses": [],
            "dns_port": 53,
            "http_port": 80,
            "protection_disabled_duration": 0,
            "protection_enabled": false,
            "protection_disabled_until": null,
            "dhcp_available": false,
            "running": true
        });

        // Mock both the POST and the subsequent GET
        Mock::given(method("POST"))
            .and(path("/control/protection"))
            .and(header("Authorization", "Basic dGVzdDp0ZXN0"))
            .respond_with(ResponseTemplate::new(200).set_body_json(status_response.clone()))
            .mount(&mock_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/control/status"))
            .and(header("Authorization", "Basic dGVzdDp0ZXN0"))
            .respond_with(ResponseTemplate::new(200).set_body_json(status_response))
            .mount(&mock_server)
            .await;

        let service = AdguardService::new(&mock_server.uri(), "test", "test", false).unwrap();
        let state = create_mock_state(Some(service));

        let response = disable_protection(State(state)).await.unwrap();
        assert!(!response.protection_enabled);
    }

    #[tokio::test]
    async fn test_pause_protection_with_mock_server() {
        let mock_server = MockServer::start().await;

        let status_response = json!({
            "version": "v1.5.0",
            "language": "en",
            "dns_addresses": [],
            "dns_port": 53,
            "http_port": 80,
            "protection_disabled_duration": 300000,
            "protection_enabled": false,
            "protection_disabled_until": "2024-01-15T10:30:00Z",
            "dhcp_available": false,
            "running": true
        });

        // Mock both the POST and the subsequent GET
        Mock::given(method("POST"))
            .and(path("/control/protection"))
            .and(header("Authorization", "Basic dGVzdDp0ZXN0"))
            .respond_with(ResponseTemplate::new(200).set_body_json(status_response.clone()))
            .mount(&mock_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/control/status"))
            .and(header("Authorization", "Basic dGVzdDp0ZXN0"))
            .respond_with(ResponseTemplate::new(200).set_body_json(status_response))
            .mount(&mock_server)
            .await;

        let service = AdguardService::new(&mock_server.uri(), "test", "test", false).unwrap();
        let state = create_mock_state(Some(service));

        let payload = PauseRequest { minutes: 5 };
        let response = pause_protection(State(state), Json(payload)).await.unwrap();
        assert!(!response.protection_enabled);
        assert_eq!(response.protection_disabled_duration, 300000);
    }

    #[tokio::test]
    async fn test_get_status_parses_all_fields_correctly() {
        let mock_server = MockServer::start().await;

        let raw_response = json!({
            "version": "v2.0.0",
            "language": "en",
            "dns_addresses": ["1.1.1.1", "8.8.8.8"],
            "dns_port": 53,
            "http_port": 80,
            "protection_disabled_duration": 0,
            "protection_enabled": true,
            "protection_disabled_until": null,
            "dhcp_available": false,
            "running": true
        });

        Mock::given(method("GET"))
            .and(path("/control/status"))
            .respond_with(ResponseTemplate::new(200).set_body_json(raw_response))
            .mount(&mock_server)
            .await;

        let service = AdguardService::new(&mock_server.uri(), "test", "test", false).unwrap();
        let state = create_mock_state(Some(service));

        let response = get_status(State(state)).await.unwrap();

        // Verify all fields are populated
        assert!(response.running);
        assert_eq!(response.version, "v2.0.0");
        assert_eq!(response.protection_disabled_duration, 0);
        assert!(response.protection_disabled_until.is_none());
    }

    #[tokio::test]
    async fn test_disable_protection_parses_disabled_fields() {
        let mock_server = MockServer::start().await;

        let status_response = json!({
            "version": "v1.5.0",
            "language": "en",
            "dns_addresses": [],
            "dns_port": 53,
            "http_port": 80,
            "protection_disabled_duration": 600,
            "protection_enabled": false,
            "protection_disabled_until": "2024-01-20T15:00:00Z",
            "dhcp_available": false,
            "running": true
        });

        // Mock both the POST and the subsequent GET
        Mock::given(method("POST"))
            .and(path("/control/protection"))
            .and(header("Authorization", "Basic dGVzdDp0ZXN0"))
            .respond_with(ResponseTemplate::new(200).set_body_json(status_response.clone()))
            .mount(&mock_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/control/status"))
            .and(header("Authorization", "Basic dGVzdDp0ZXN0"))
            .respond_with(ResponseTemplate::new(200).set_body_json(status_response))
            .mount(&mock_server)
            .await;

        let service = AdguardService::new(&mock_server.uri(), "test", "test", false).unwrap();
        let state = create_mock_state(Some(service));

        let response = disable_protection(State(state)).await.unwrap();

        // Verify disabled status fields
        assert!(!response.protection_enabled);
        assert_eq!(response.protection_disabled_duration, 600);
        assert_eq!(
            response.protection_disabled_until,
            Some("2024-01-20T15:00:00Z".to_string())
        );
    }
}
