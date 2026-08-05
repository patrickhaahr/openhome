use axum::{
    Json, Router,
    extract::State,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, Result};
use crate::services::ir::{IrRemote, IrService, IrServiceError, IrStatusResponse};

#[derive(Debug, Deserialize)]
pub struct SendCommandRequest {
    pub command: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SendCommandResponse {
    pub message: String,
}

pub fn router() -> Router<crate::AppState> {
    Router::new()
        .route("/api/ir", get(get_status))
        .route("/api/ir/edifier", post(send_edifier_command))
        .route("/api/ir/lgtv", post(send_lgtv_command))
}

async fn get_status(State(state): State<crate::AppState>) -> Result<Json<IrStatusResponse>> {
    let service = ir_service(&state)?;
    let status = service.get_status().await.map_err(map_ir_error)?;

    Ok(Json(status))
}

async fn send_edifier_command(
    State(state): State<crate::AppState>,
    Json(payload): Json<SendCommandRequest>,
) -> Result<Json<SendCommandResponse>> {
    send_command(state, payload, IrRemote::Edifier).await
}

async fn send_lgtv_command(
    State(state): State<crate::AppState>,
    Json(payload): Json<SendCommandRequest>,
) -> Result<Json<SendCommandResponse>> {
    send_command(state, payload, IrRemote::LgTv).await
}

async fn send_command(
    state: crate::AppState,
    payload: SendCommandRequest,
    remote: IrRemote,
) -> Result<Json<SendCommandResponse>> {
    let command = payload
        .command
        .as_deref()
        .map(str::trim)
        .filter(|command| !command.is_empty())
        .ok_or_else(|| AppError::Validation("command is required".to_string()))?;
    let service = ir_service(&state)?;

    let message = service
        .send_command(remote, command)
        .await
        .map_err(map_ir_error)?;

    Ok(Json(SendCommandResponse { message }))
}

fn ir_service(state: &crate::AppState) -> Result<&IrService> {
    state
        .ir_service
        .as_ref()
        .ok_or_else(|| AppError::ServiceUnavailable("IR service is not configured".to_string()))
}

fn map_ir_error(error: IrServiceError) -> AppError {
    match error {
        IrServiceError::Validation(message) => AppError::Validation(message),
        IrServiceError::NotFound(message) => AppError::NotFound(message),
        IrServiceError::ServiceUnavailable(message) => AppError::ServiceUnavailable(message),
        IrServiceError::UnexpectedStatus { status, message } => AppError::Internal(
            anyhow::anyhow!("IR device returned status {status}: {message}"),
        ),
        IrServiceError::Request(error) => {
            AppError::ServiceUnavailable(format!("Failed to contact IR device: {error}"))
        }
    }
}
