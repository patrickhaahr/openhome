use axum::{
    Extension, Json, Router,
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};
use serde_json::json;

use crate::{
    AppState,
    error::AppError,
    services::{
        binder::{self, AddEntry, Addition, BinderEntry, BinderError, EntriesFilter},
        optcg::Optcg,
    },
};

pub fn router(catalogue: Optcg) -> Router<AppState> {
    Router::new()
        .route("/api/binder/entries", get(entries).post(add_entry))
        .layer(Extension(catalogue))
}

async fn entries(
    State(state): State<AppState>,
    Query(filter): Query<EntriesFilter>,
) -> Result<Json<Vec<BinderEntry>>, BinderError> {
    Ok(Json(binder::entries(&state.db, filter).await?))
}

async fn add_entry(
    State(state): State<AppState>,
    Extension(catalogue): Extension<Optcg>,
    Json(request): Json<AddEntry>,
) -> Result<(StatusCode, Json<Addition>), BinderError> {
    let addition = binder::add_entry(&state.db, &catalogue, request).await?;
    let status = if addition.replayed {
        StatusCode::OK
    } else {
        StatusCode::CREATED
    };
    Ok((status, Json(addition)))
}

impl IntoResponse for BinderError {
    fn into_response(self) -> Response {
        match self {
            Self::Validation(message) => AppError::Validation(message).into_response(),
            Self::NotFound(message) => AppError::NotFound(message).into_response(),
            Self::Conflict(message) => AppError::Conflict(message).into_response(),
            Self::DestinationRequired { choices } => (
                StatusCode::CONFLICT,
                Json(json!({
                    "error": "Choose a Household User before adding a Binder Entry",
                    "status": 409,
                    "code": "binder_destination_required",
                    "question": "Which household owner's Binder should receive this card?",
                    "choices": choices,
                })),
            )
                .into_response(),
            Self::NotListed => (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "error": "The catalogue does not list this card; search_cards its Card Code to see its variants",
                    "status": 404,
                    "code": "card_not_listed",
                })),
            )
                .into_response(),
            Self::Cards(error) => error.into_response(),
            Self::Database(error) => AppError::Internal(error.into()).into_response(),
        }
    }
}
