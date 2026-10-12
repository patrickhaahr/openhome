use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};
use serde_json::json;

use crate::{
    AppState,
    error::AppError,
    services::watchlist::{self, AddEntry, Addition, WatchlistEntry, WatchlistError},
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/watchlist/entries", get(entries).post(add_entry))
        .route(
            "/api/household-users/{owner_id}/watchlist/entries/{id}",
            get(entry).delete(remove_entry),
        )
}

async fn entries(
    State(state): State<AppState>,
) -> Result<Json<Vec<WatchlistEntry>>, WatchlistError> {
    Ok(Json(watchlist::entries(&state.db).await?))
}

async fn add_entry(
    State(state): State<AppState>,
    Json(request): Json<AddEntry>,
) -> Result<(StatusCode, Json<Addition>), WatchlistError> {
    let addition = watchlist::add_entry(&state.db, request).await?;
    let status = if addition.replayed {
        StatusCode::OK
    } else {
        StatusCode::CREATED
    };
    Ok((status, Json(addition)))
}

async fn entry(
    State(state): State<AppState>,
    Path((owner_id, id)): Path<(String, String)>,
) -> Result<Json<WatchlistEntry>, WatchlistError> {
    Ok(Json(watchlist::entry(&state.db, &owner_id, &id).await?))
}

async fn remove_entry(
    State(state): State<AppState>,
    Path((owner_id, id)): Path<(String, String)>,
) -> Result<StatusCode, WatchlistError> {
    watchlist::remove_entry(&state.db, &owner_id, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}

impl IntoResponse for WatchlistError {
    fn into_response(self) -> Response {
        match self {
            Self::Validation(message) => AppError::Validation(message).into_response(),
            Self::NotFound(message) => AppError::NotFound(message).into_response(),
            Self::Conflict(message) => AppError::Conflict(message).into_response(),
            Self::DestinationRequired { choices } => (
                StatusCode::CONFLICT,
                Json(json!({
                    "error": "Choose a Household User before adding a Watchlist Entry",
                    "status": 409,
                    "code": "watchlist_destination_required",
                    "question": "Which household owner's watchlist should receive this entry?",
                    "choices": choices,
                })),
            )
                .into_response(),
            Self::Database(sqlx::Error::Database(error)) if error.is_unique_violation() => {
                AppError::Conflict("This request is already in use".into()).into_response()
            }
            Self::Database(error) => AppError::Internal(error.into()).into_response(),
        }
    }
}
