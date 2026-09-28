use axum::{Json, http::StatusCode, response::IntoResponse};
use serde::Serialize;
use thiserror::Error;

#[derive(Error, Debug)]
#[allow(dead_code)]
pub enum AppError {
    #[error("Unauthorized")]
    Unauthorized(String),
    #[error("Not found")]
    NotFound(String),
    #[error("Conflict")]
    Conflict(String),
    #[error("Validation error")]
    Validation(String),
    #[error("Unprocessable entity")]
    Unprocessable(String),
    #[error("Service unavailable")]
    ServiceUnavailable(String),
    #[error("Internal server error")]
    Internal(#[from] anyhow::Error),
    #[error("Docker error: {0}")]
    DockerError(String),
    #[error("Container not found: {0}")]
    ContainerNotFound(String),
}

#[derive(Serialize)]
#[allow(dead_code)]
struct ErrorResponse {
    error: String,
    status: u16,
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match &self {
            Self::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, msg.clone()),
            Self::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            Self::Conflict(msg) => (StatusCode::CONFLICT, msg.clone()),
            Self::Validation(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            Self::Unprocessable(msg) => (StatusCode::UNPROCESSABLE_ENTITY, msg.clone()),
            Self::ServiceUnavailable(msg) => (StatusCode::SERVICE_UNAVAILABLE, msg.clone()),
            Self::Internal(err) => {
                tracing::error!(error = ?err, "Internal server error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "An internal error occurred".to_string(),
                )
            }
            Self::DockerError(msg) => {
                tracing::error!(error = %msg, "Docker error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Docker daemon error".to_string(),
                )
            }
            Self::ContainerNotFound(name) => {
                tracing::warn!(container = %name, "Container not found");
                (
                    StatusCode::NOT_FOUND,
                    format!("Container '{name}' not found"),
                )
            }
        };

        let body = ErrorResponse {
            error: message,
            status: status.as_u16(),
        };
        (status, Json(body)).into_response()
    }
}

#[allow(dead_code)]
pub type Result<T> = std::result::Result<T, AppError>;
