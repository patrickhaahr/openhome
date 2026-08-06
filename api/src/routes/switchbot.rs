use axum::{Json, Router, extract::State, routing::post};

use crate::error::{AppError, Result};
use crate::services::switchbot::{LightCommand, SwitchbotMessageResponse, SwitchbotServiceError};

pub fn router() -> Router<crate::AppState> {
    Router::new()
        .route("/api/lights/on", post(switch_light_on))
        .route("/api/lights/off", post(switch_light_off))
}

async fn switch_light_on(
    State(state): State<crate::AppState>,
) -> Result<Json<SwitchbotMessageResponse>> {
    send_light_command(&state, LightCommand::On).await
}

async fn switch_light_off(
    State(state): State<crate::AppState>,
) -> Result<Json<SwitchbotMessageResponse>> {
    send_light_command(&state, LightCommand::Off).await
}

async fn send_light_command(
    state: &crate::AppState,
    command: LightCommand,
) -> Result<Json<SwitchbotMessageResponse>> {
    let service = state.switchbot_service.as_ref().ok_or_else(|| {
        AppError::ServiceUnavailable("SwitchBot service is not configured".to_string())
    })?;
    let response = service
        .send_light_command(command)
        .await
        .map_err(map_switchbot_error)?;

    Ok(Json(response))
}

fn map_switchbot_error(error: SwitchbotServiceError) -> AppError {
    match error {
        SwitchbotServiceError::ServiceUnavailable(message) => AppError::ServiceUnavailable(message),
        SwitchbotServiceError::UnexpectedStatus { status, message } => AppError::Internal(
            anyhow::anyhow!("SwitchBot returned status {status}: {message}"),
        ),
        SwitchbotServiceError::Request(error) => {
            AppError::ServiceUnavailable(format!("Failed to contact SwitchBot: {error}"))
        }
    }
}
