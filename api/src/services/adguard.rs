use std::time::Duration;

use base64::Engine;
use reqwest::header::HeaderMap;
use reqwest::{Client, ClientBuilder};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct AdguardService {
    client: Client,
    base_url: String,
}

impl AdguardService {
    pub fn new(
        host: &str,
        username: &str,
        password: &str,
        insecure_tls: bool,
    ) -> Result<Self, anyhow::Error> {
        let auth_header = format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode(format!("{}:{}", username, password))
        );

        let mut headers = HeaderMap::new();
        let auth_value = auth_header
            .parse()
            .map_err(|e| anyhow::anyhow!("Invalid AdGuard auth header: {e}"))?;
        headers.insert(reqwest::header::AUTHORIZATION, auth_value);

        let mut client_builder = ClientBuilder::new()
            .timeout(Duration::from_secs(30))
            .default_headers(headers);

        if insecure_tls {
            client_builder = client_builder.danger_accept_invalid_certs(true);
        }

        let client = client_builder.build()?;

        let base_url = host.trim_end_matches('/').to_string();

        Ok(Self { client, base_url })
    }

    pub async fn get_status(&self) -> Result<AdguardStatusResponse, anyhow::Error> {
        let url = format!("{}/control/status", self.base_url);
        let raw: RawAdguardStatusResponse = self
            .client
            .get(&url)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(AdguardStatusResponse {
            protection_enabled: raw.protection_enabled,
            protection_disabled_duration: raw.protection_disabled_duration,
            protection_disabled_until: raw.protection_disabled_until,
            version: raw.version,
            running: raw.running,
        })
    }

    pub async fn set_protection(
        &self,
        enabled: bool,
        duration_ms: Option<u64>,
    ) -> Result<AdguardStatusResponse, anyhow::Error> {
        let url = format!("{}/control/protection", self.base_url);
        let mut body = serde_json::json!({
            "enabled": enabled
        });
        if let Some(duration) = duration_ms {
            body["duration"] = duration.into();
        }

        let response = self.client.post(&url).json(&body).send().await?;
        let status = response.status();
        if !status.is_success() {
            let response_body = response.text().await?;
            return Err(anyhow::anyhow!(
                "AdGuard returned status {}: {}",
                status,
                response_body
            ));
        }

        self.get_status().await
    }
}

#[derive(Debug, Serialize)]
pub struct AdguardStatusResponse {
    pub protection_enabled: bool,
    pub protection_disabled_duration: i64,
    pub protection_disabled_until: Option<String>,
    pub version: String,
    pub running: bool,
}

#[derive(Debug, Deserialize)]
struct RawAdguardStatusResponse {
    version: String,
    protection_disabled_duration: i64,
    protection_enabled: bool,
    protection_disabled_until: Option<String>,
    running: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn create_test_service_with_url(base_url: &str) -> AdguardService {
        AdguardService::new(base_url, "test", "test", false).unwrap()
    }

    #[tokio::test]
    async fn test_get_status_parses_response_correctly() {
        let mock_server = MockServer::start().await;
        let raw_response = json!({
            "version": "v1.5.0",
            "language": "en",
            "dns_addresses": ["8.8.8.8", "8.8.4.4"],
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

        let service = create_test_service_with_url(&mock_server.uri());
        let status = service.get_status().await.unwrap();

        assert!(status.protection_enabled);
        assert_eq!(status.version, "v1.5.0");
        assert!(status.running);
        assert_eq!(status.protection_disabled_duration, 0);
        assert!(status.protection_disabled_until.is_none());
    }

    #[tokio::test]
    async fn test_get_status_handles_disabled_state() {
        let mock_server = MockServer::start().await;
        let raw_response = json!({
            "version": "v1.5.0",
            "language": "en",
            "dns_addresses": ["8.8.8.8"],
            "dns_port": 53,
            "http_port": 80,
            "protection_disabled_duration": 300,
            "protection_enabled": false,
            "protection_disabled_until": "2024-01-15T10:30:00Z",
            "dhcp_available": false,
            "running": true
        });

        Mock::given(method("GET"))
            .and(path("/control/status"))
            .and(header("Authorization", "Basic dGVzdDp0ZXN0"))
            .respond_with(ResponseTemplate::new(200).set_body_json(raw_response))
            .mount(&mock_server)
            .await;

        let service = create_test_service_with_url(&mock_server.uri());
        let status = service.get_status().await.unwrap();

        assert!(!status.protection_enabled);
        assert_eq!(status.protection_disabled_duration, 300);
        assert_eq!(
            status.protection_disabled_until,
            Some("2024-01-15T10:30:00Z".to_string())
        );
    }

    #[tokio::test]
    async fn test_set_protection_builds_correct_enable_body() {
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
            .respond_with(ResponseTemplate::new(200).set_body_json(status_response))
            .mount(&mock_server)
            .await;

        let service = create_test_service_with_url(&mock_server.uri());
        let result = service.set_protection(true, None).await.unwrap();

        assert!(result.protection_enabled);
    }

    #[tokio::test]
    async fn test_set_protection_builds_correct_disable_body() {
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
            .respond_with(ResponseTemplate::new(200).set_body_json(status_response.clone()))
            .mount(&mock_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/control/status"))
            .respond_with(ResponseTemplate::new(200).set_body_json(status_response))
            .mount(&mock_server)
            .await;

        let service = create_test_service_with_url(&mock_server.uri());
        let result = service.set_protection(false, None).await.unwrap();

        assert!(!result.protection_enabled);
    }

    #[tokio::test]
    async fn test_set_protection_with_duration() {
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
            .respond_with(ResponseTemplate::new(200).set_body_json(status_response.clone()))
            .mount(&mock_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/control/status"))
            .respond_with(ResponseTemplate::new(200).set_body_json(status_response))
            .mount(&mock_server)
            .await;

        let service = create_test_service_with_url(&mock_server.uri());
        let result = service.set_protection(false, Some(300000)).await.unwrap();

        assert!(!result.protection_enabled);
        assert_eq!(result.protection_disabled_duration, 300000);
    }

    #[tokio::test]
    async fn test_set_protection_handles_error_response() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/control/protection"))
            .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
            .mount(&mock_server)
            .await;

        let service = create_test_service_with_url(&mock_server.uri());
        let result = service.set_protection(true, None).await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("500"));
    }

    #[tokio::test]
    async fn test_new_strips_trailing_slash() {
        let service = AdguardService::new("http://localhost:3000/", "user", "pass", false).unwrap();
        assert_eq!(service.base_url, "http://localhost:3000");
    }

    #[tokio::test]
    async fn test_get_status_handles_json_error() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/control/status"))
            .respond_with(ResponseTemplate::new(200).set_body_string("not valid json"))
            .mount(&mock_server)
            .await;

        let service = create_test_service_with_url(&mock_server.uri());
        let result = service.get_status().await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_status_handles_network_error() {
        let service = create_test_service_with_url("http://nonexistent:9999");
        let result = service.get_status().await;

        assert!(result.is_err());
    }
}
