use axum::{
    Extension, Json, Router,
    extract::{State, rejection::JsonRejection},
    routing::{get, post},
};

use crate::{
    AppState,
    error::AppError,
    services::{
        active_route::{self, ActiveRouteError, ActiveRouteRead, Selection},
        route_loop::{LoopPlanner, LoopRead, LoopRequest, LoopRequestBody},
    },
};

/// Generate and select loops through the authenticated Axum boundary.
pub fn router(planner: LoopPlanner) -> Router<crate::AppState> {
    Router::new()
        .route("/api/routes/loops", post(generate))
        .route("/api/routes/active", get(read_active).put(select_active))
        .layer(Extension(planner))
        .layer(axum::middleware::map_response(no_store))
}

async fn generate(
    Extension(planner): Extension<LoopPlanner>,
    State(state): State<AppState>,
    body: Result<Json<LoopRequestBody>, JsonRejection>,
) -> Result<Json<LoopRead>, AppError> {
    let Json(body) = body.map_err(|_| AppError::Validation("Invalid loop request".to_string()))?;
    let request =
        LoopRequest::parse(&body).map_err(|error| AppError::Validation(error.to_string()))?;
    let read = planner.generate(request).await;
    if let LoopRead::Generated { plan } = &read {
        active_route::remember(&state.db, plan)
            .await
            .map_err(map_error)?;
    }
    Ok(Json(read))
}

async fn read_active(State(state): State<AppState>) -> Result<Json<ActiveRouteRead>, AppError> {
    active_route::read(&state.db)
        .await
        .map(Json)
        .map_err(map_error)
}

async fn select_active(
    State(state): State<AppState>,
    body: Result<Json<Selection>, JsonRejection>,
) -> Result<Json<ActiveRouteRead>, AppError> {
    let Json(selection) =
        body.map_err(|_| AppError::Validation("Invalid route selection".to_string()))?;
    active_route::select(&state.db, selection)
        .await
        .map(Json)
        .map_err(map_error)
}

fn map_error(error: ActiveRouteError) -> AppError {
    match error {
        ActiveRouteError::CandidateUnavailable => AppError::Conflict(error.to_string()),
        ActiveRouteError::Database(_) | ActiveRouteError::Snapshot(_) => {
            AppError::Internal(error.into())
        }
    }
}

async fn no_store(mut response: axum::response::Response) -> axum::response::Response {
    response.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store"),
    );
    response
}
