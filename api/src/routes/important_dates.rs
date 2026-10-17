use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};

use crate::{
    AppState,
    error::AppError,
    services::important_dates::{
        self, AddDate, Addition, DatesQuery, EditDate, ImportantDate, ImportantDateError,
        ImportantDates, OnQuery,
    },
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/important-dates", get(dates).post(add_date))
        .route(
            "/api/important-dates/{id}",
            get(date).patch(edit_date).delete(remove_date),
        )
}

async fn dates(
    State(state): State<AppState>,
    Query(query): Query<DatesQuery>,
) -> Result<Json<ImportantDates>, ImportantDateError> {
    Ok(Json(important_dates::dates(&state.db, &query).await?))
}

async fn date(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<OnQuery>,
) -> Result<Json<ImportantDate>, ImportantDateError> {
    let on = important_dates::resolve_on(query.on.as_deref())?;
    Ok(Json(important_dates::date(&state.db, &id, on).await?))
}

async fn add_date(
    State(state): State<AppState>,
    Json(request): Json<AddDate>,
) -> Result<(StatusCode, Json<Addition>), ImportantDateError> {
    let addition = important_dates::add_date(&state.db, request).await?;
    let status = if addition.replayed {
        StatusCode::OK
    } else {
        StatusCode::CREATED
    };
    Ok((status, Json(addition)))
}

async fn edit_date(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(request): Json<EditDate>,
) -> Result<Json<ImportantDate>, ImportantDateError> {
    Ok(Json(
        important_dates::edit_date(&state.db, &id, request).await?,
    ))
}

async fn remove_date(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ImportantDateError> {
    important_dates::remove_date(&state.db, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}

impl IntoResponse for ImportantDateError {
    fn into_response(self) -> Response {
        match self {
            Self::Validation(message) => AppError::Validation(message).into_response(),
            Self::NotFound(message) => AppError::NotFound(message).into_response(),
            Self::Conflict(message) => AppError::Conflict(message).into_response(),
            Self::Database(sqlx::Error::Database(error)) if error.is_unique_violation() => {
                AppError::Conflict("This request is already in use".into()).into_response()
            }
            Self::Database(error) => AppError::Internal(error.into()).into_response(),
        }
    }
}
