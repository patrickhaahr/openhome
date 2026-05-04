use std::time::Duration;

use reqwest::{Client, ClientBuilder, StatusCode};
use serde::Serialize;
use url::Url;

#[derive(Debug, Clone)]
pub struct IrService {
    client: Client,
    base_url: String,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct IrStatusResponse {
    pub message: String,
    pub available_commands: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum IrServiceError {
    #[error("{0}")]
    Validation(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    ServiceUnavailable(String),
    #[error("IR device returned status {status}: {message}")]
    UnexpectedStatus { status: StatusCode, message: String },
    #[error(transparent)]
    Request(#[from] reqwest::Error),
}

impl IrService {
    pub fn new(base_url: &str) -> Result<Self, anyhow::Error> {
        let base_url = base_url.trim_end_matches('/').to_string();
        Url::parse(&base_url)?;

        let client = ClientBuilder::new()
            .timeout(Duration::from_secs(10))
            .build()?;

        Ok(Self { client, base_url })
    }

    pub async fn get_status(&self) -> Result<IrStatusResponse, IrServiceError> {
        let url = format!("{}/", self.base_url);
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(map_request_error)?;
        let body = read_response_text(response).await?;

        Ok(parse_status_response(&body))
    }

    pub async fn send_command(&self, command: &str) -> Result<String, IrServiceError> {
        let mut url = Url::parse(&format!("{}/send", self.base_url)).map_err(|error| {
            IrServiceError::ServiceUnavailable(format!("Invalid IR device URL: {error}"))
        })?;
        url.query_pairs_mut().append_pair("command", command);

        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(map_request_error)?;

        read_response_text(response).await
    }
}

fn map_request_error(error: reqwest::Error) -> IrServiceError {
    if error.is_connect() || error.is_timeout() {
        return IrServiceError::ServiceUnavailable(error.to_string());
    }

    IrServiceError::Request(error)
}

async fn read_response_text(response: reqwest::Response) -> Result<String, IrServiceError> {
    let status = response.status();

    if status.is_success() {
        return response.text().await.map_err(map_request_error);
    }

    let message = response.text().await.unwrap_or_default();

    Err(map_response_error(status, message))
}

fn map_response_error(status: StatusCode, message: String) -> IrServiceError {
    match status {
        StatusCode::BAD_REQUEST => IrServiceError::Validation(message),
        StatusCode::NOT_FOUND => IrServiceError::NotFound(message),
        StatusCode::SERVICE_UNAVAILABLE | StatusCode::BAD_GATEWAY | StatusCode::GATEWAY_TIMEOUT => {
            IrServiceError::ServiceUnavailable(message)
        }
        _ => IrServiceError::UnexpectedStatus { status, message },
    }
}

fn parse_status_response(body: &str) -> IrStatusResponse {
    let message = body
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("IR remote ready")
        .to_string();

    let available_commands = body
        .lines()
        .find_map(|line| line.strip_prefix("Available commands: "))
        .map(|line| {
            line.split(',')
                .map(str::trim)
                .filter(|command| !command.is_empty())
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default();

    IrStatusResponse {
        message,
        available_commands,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_get_status_parses_device_response() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                "IR remote ready\n\nAvailable commands: bluetooth, optical, mute, volume-up\n",
            ))
            .mount(&mock_server)
            .await;

        let service = IrService::new(&mock_server.uri()).unwrap();
        let response = service.get_status().await.unwrap();

        assert_eq!(
            response,
            IrStatusResponse {
                message: "IR remote ready".to_string(),
                available_commands: vec![
                    "bluetooth".to_string(),
                    "optical".to_string(),
                    "mute".to_string(),
                    "volume-up".to_string(),
                ],
            }
        );
    }

    #[tokio::test]
    async fn test_send_command_passes_command_query_parameter() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/send"))
            .and(query_param("command", "mute"))
            .respond_with(ResponseTemplate::new(200).set_body_string("Sent command: mute"))
            .mount(&mock_server)
            .await;

        let service = IrService::new(&mock_server.uri()).unwrap();
        let response = service.send_command("mute").await.unwrap();

        assert_eq!(response, "Sent command: mute");
    }

    #[tokio::test]
    async fn test_get_status_maps_service_unavailable_errors() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(503).set_body_string("IR bridge offline"))
            .mount(&mock_server)
            .await;

        let service = IrService::new(&mock_server.uri()).unwrap();
        let error = service.get_status().await.unwrap_err();

        assert!(
            matches!(error, IrServiceError::ServiceUnavailable(message) if message == "IR bridge offline")
        );
    }

    #[tokio::test]
    async fn test_send_command_maps_validation_errors() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/send"))
            .respond_with(ResponseTemplate::new(400).set_body_string("Missing command parameter"))
            .mount(&mock_server)
            .await;

        let service = IrService::new(&mock_server.uri()).unwrap();
        let error = service.send_command("").await.unwrap_err();

        assert!(
            matches!(error, IrServiceError::Validation(message) if message == "Missing command parameter")
        );
    }

    #[tokio::test]
    async fn test_send_command_maps_not_found_errors() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/send"))
            .respond_with(ResponseTemplate::new(404).set_body_string("Unknown command 'party'"))
            .mount(&mock_server)
            .await;

        let service = IrService::new(&mock_server.uri()).unwrap();
        let error = service.send_command("party").await.unwrap_err();

        assert!(
            matches!(error, IrServiceError::NotFound(message) if message == "Unknown command 'party'")
        );
    }

    #[tokio::test]
    async fn test_new_strips_trailing_slash() {
        let service = IrService::new("http://localhost:3000/").unwrap();
        assert_eq!(service.base_url, "http://localhost:3000");
    }
}
