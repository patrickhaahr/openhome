use std::time::Duration;

use reqwest::{Client, ClientBuilder, StatusCode};
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Debug, Clone)]
pub struct IrService {
    client: Client,
    base_url: String,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct IrStatusResponse {
    #[serde(default = "default_ready_message")]
    pub message: String,
    pub remotes: IrRemotesResponse,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct IrRemotesResponse {
    pub edifier: Vec<String>,
    pub lgtv: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
pub enum IrRemote {
    Edifier,
    LgTv,
}

impl IrRemote {
    fn as_str(self) -> &'static str {
        match self {
            Self::Edifier => "edifier",
            Self::LgTv => "lgtv",
        }
    }
}

#[derive(Debug, Deserialize)]
struct IrErrorResponse {
    error: String,
}

#[derive(Debug, Deserialize)]
struct IrMessageResponse {
    message: String,
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
        let url = format!("{}/remotes", self.base_url);
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(map_request_error)?;
        read_response_json(response).await
    }

    pub async fn send_command(
        &self,
        remote: IrRemote,
        command: &str,
    ) -> Result<String, IrServiceError> {
        let url = Url::parse(&format!("{}/send", self.base_url)).map_err(|error| {
            IrServiceError::ServiceUnavailable(format!("Invalid IR device URL: {error}"))
        })?;

        let response = self
            .client
            .post(url)
            .form(&[("remote", remote.as_str()), ("command", command)])
            .send()
            .await
            .map_err(map_request_error)?;

        Ok(read_response_json::<IrMessageResponse>(response)
            .await?
            .message)
    }
}

fn map_request_error(error: reqwest::Error) -> IrServiceError {
    if error.is_connect() || error.is_timeout() {
        return IrServiceError::ServiceUnavailable(error.to_string());
    }

    IrServiceError::Request(error)
}

async fn read_response_json<T>(response: reqwest::Response) -> Result<T, IrServiceError>
where
    T: for<'de> Deserialize<'de>,
{
    let status = response.status();
    if status.is_success() {
        return response.json().await.map_err(map_request_error);
    }

    let body = response.text().await.unwrap_or_default();
    let message = serde_json::from_str::<IrErrorResponse>(&body)
        .map(|response| response.error)
        .unwrap_or(body);

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

fn default_ready_message() -> String {
    "IR remote ready".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{body_string_contains, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_get_status_parses_device_response() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/remotes"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "remotes": {
                    "edifier": ["bluetooth", "optical", "mute", "volume-up"],
                    "lgtv": ["power", "hdmi-1"]
                }
            })))
            .mount(&mock_server)
            .await;

        let service = IrService::new(&mock_server.uri()).unwrap();
        let response = service.get_status().await.unwrap();

        assert_eq!(
            response,
            IrStatusResponse {
                message: "IR remote ready".to_string(),
                remotes: IrRemotesResponse {
                    edifier: vec![
                        "bluetooth".to_string(),
                        "optical".to_string(),
                        "mute".to_string(),
                        "volume-up".to_string(),
                    ],
                    lgtv: vec!["power".to_string(), "hdmi-1".to_string()],
                },
            }
        );
    }

    #[tokio::test]
    async fn test_send_command_posts_remote_and_command_form_fields() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/send"))
            .and(body_string_contains("remote=edifier"))
            .and(body_string_contains("command=mute"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({ "message": "Sent command: mute" })),
            )
            .mount(&mock_server)
            .await;

        let service = IrService::new(&mock_server.uri()).unwrap();
        let response = service
            .send_command(IrRemote::Edifier, "mute")
            .await
            .unwrap();

        assert_eq!(response, "Sent command: mute");
    }

    #[tokio::test]
    async fn test_get_status_maps_service_unavailable_errors() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/remotes"))
            .respond_with(
                ResponseTemplate::new(503)
                    .set_body_json(serde_json::json!({ "error": "IR bridge offline" })),
            )
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

        Mock::given(method("POST"))
            .and(path("/send"))
            .respond_with(
                ResponseTemplate::new(400)
                    .set_body_json(serde_json::json!({ "error": "Missing form field: command" })),
            )
            .mount(&mock_server)
            .await;

        let service = IrService::new(&mock_server.uri()).unwrap();
        let error = service
            .send_command(IrRemote::Edifier, "")
            .await
            .unwrap_err();

        assert!(
            matches!(error, IrServiceError::Validation(message) if message == "Missing form field: command")
        );
    }

    #[tokio::test]
    async fn test_send_command_maps_not_found_errors() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/send"))
            .respond_with(
                ResponseTemplate::new(404)
                    .set_body_json(serde_json::json!({ "error": "Unknown command" })),
            )
            .mount(&mock_server)
            .await;

        let service = IrService::new(&mock_server.uri()).unwrap();
        let error = service
            .send_command(IrRemote::Edifier, "party")
            .await
            .unwrap_err();

        assert!(matches!(error, IrServiceError::NotFound(message) if message == "Unknown command"));
    }

    #[tokio::test]
    async fn test_new_strips_trailing_slash() {
        let service = IrService::new("http://localhost:3000/").unwrap();
        assert_eq!(service.base_url, "http://localhost:3000");
    }
}
