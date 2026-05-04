use axum::{
    Json, Router,
    extract::{Query, State},
    routing::get,
};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, Result};
use crate::services::ir::{IrService, IrServiceError, IrStatusResponse};

#[derive(Debug, Deserialize)]
pub struct SendCommandRequest {
    pub command: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SendCommandResponse {
    pub message: String,
}

pub fn router() -> Router<crate::AppState> {
    Router::new().route("/api/ir", get(get_status)).route(
        "/api/ir/send",
        get(send_command_from_query).post(send_command_from_json),
    )
}

async fn get_status(State(state): State<crate::AppState>) -> Result<Json<IrStatusResponse>> {
    let service = ir_service(&state)?;
    let status = service.get_status().await.map_err(map_ir_error)?;

    Ok(Json(status))
}

async fn send_command_from_query(
    State(state): State<crate::AppState>,
    Query(payload): Query<SendCommandRequest>,
) -> Result<Json<SendCommandResponse>> {
    send_command(state, payload).await
}

async fn send_command_from_json(
    State(state): State<crate::AppState>,
    Json(payload): Json<SendCommandRequest>,
) -> Result<Json<SendCommandResponse>> {
    send_command(state, payload).await
}

async fn send_command(
    state: crate::AppState,
    payload: SendCommandRequest,
) -> Result<Json<SendCommandResponse>> {
    let command = payload
        .command
        .as_deref()
        .map(str::trim)
        .filter(|command| !command.is_empty())
        .ok_or_else(|| AppError::Validation("command is required".to_string()))?;
    let service = ir_service(&state)?;

    let message = service.send_command(command).await.map_err(map_ir_error)?;

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
