use axum::{
    Json, Router,
    extract::{State, rejection::JsonRejection},
    routing::post,
};

use crate::{
    error::AppError,
    services::route_loop::{LoopPlanner, LoopRead, LoopRequest, LoopRequestBody},
};

/// Loop generation owns its provider gate; it needs no database state.
pub fn router(planner: LoopPlanner) -> Router<crate::AppState> {
    Router::new()
        .route("/api/routes/loops", post(generate))
        .with_state(planner)
}

async fn generate(
    State(planner): State<LoopPlanner>,
    body: Result<Json<LoopRequestBody>, JsonRejection>,
) -> Result<Json<LoopRead>, AppError> {
    let Json(body) = body.map_err(|_| AppError::Validation("Invalid loop request".to_string()))?;
    let request =
        LoopRequest::parse(&body).map_err(|error| AppError::Validation(error.to_string()))?;
    Ok(Json(planner.generate(request).await))
}
