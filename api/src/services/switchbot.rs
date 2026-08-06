use std::time::Duration;

use reqwest::{Client, ClientBuilder, StatusCode};
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Debug, Clone)]
pub struct SwitchbotService {
    client: Client,
    base_url: String,
}

#[derive(Debug, Clone, Copy)]
pub enum LightCommand {
    On,
    Off,
}

impl LightCommand {
    fn as_str(self) -> &'static str {
        match self {
            Self::On => "on",
            Self::Off => "off",
        }
    }
}

#[derive(Debug, Deserialize)]
struct SwitchbotErrorResponse {
    error: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SwitchbotMessageResponse {
    pub message: String,
}

#[derive(Debug, thiserror::Error)]
pub enum SwitchbotServiceError {
    #[error("{0}")]
    ServiceUnavailable(String),
    #[error("SwitchBot returned status {status}: {message}")]
    UnexpectedStatus { status: StatusCode, message: String },
    #[error(transparent)]
    Request(#[from] reqwest::Error),
}

impl SwitchbotService {
    pub fn new(base_url: &str) -> Result<Self, anyhow::Error> {
        let base_url = base_url.trim_end_matches('/').to_string();
        let parsed_url = Url::parse(&base_url)?;
        if !matches!(parsed_url.scheme(), "http" | "https") || parsed_url.host().is_none() {
            anyhow::bail!("SwitchBot base URL must be an HTTP or HTTPS URL with a host");
        }

        let client = ClientBuilder::new()
            .timeout(Duration::from_secs(10))
            .build()?;

        Ok(Self { client, base_url })
    }

    pub async fn send_light_command(
        &self,
        command: LightCommand,
    ) -> Result<SwitchbotMessageResponse, SwitchbotServiceError> {
        let url = format!("{}/lights/{}", self.base_url, command.as_str());
        let response = self
            .client
            .post(url)
            .send()
            .await
            .map_err(map_request_error)?;

        read_response_json(response).await
    }
}

fn map_request_error(error: reqwest::Error) -> SwitchbotServiceError {
    if error.is_connect() || error.is_timeout() {
        return SwitchbotServiceError::ServiceUnavailable(error.to_string());
    }

    SwitchbotServiceError::Request(error)
}

async fn read_response_json(
    response: reqwest::Response,
) -> Result<SwitchbotMessageResponse, SwitchbotServiceError> {
    let status = response.status();
    if status.is_success() {
        return response.json().await.map_err(map_request_error);
    }

    let body = response.text().await.unwrap_or_default();
    let message = serde_json::from_str::<SwitchbotErrorResponse>(&body)
        .map(|response| response.error)
        .unwrap_or(body);

    if matches!(
        status,
        StatusCode::SERVICE_UNAVAILABLE | StatusCode::BAD_GATEWAY | StatusCode::GATEWAY_TIMEOUT
    ) {
        return Err(SwitchbotServiceError::ServiceUnavailable(message));
    }

    Err(SwitchbotServiceError::UnexpectedStatus { status, message })
}
