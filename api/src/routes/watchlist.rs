use crate::services::{
    tmdb::{MediaMetadata, TmdbService},
    watchlist::MediaKind,
};
use axum::{
    Extension, Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};
use serde::Deserialize;
use serde_json::json;

use crate::{
    AppState,
    error::AppError,
    services::watchlist::{
        self, AddEntry, Addition, EditEntry, EntriesFilter, WatchlistEntry, WatchlistError,
    },
};

pub fn router(provider: Option<TmdbService>) -> Router<AppState> {
    Router::new()
        .route("/api/watchlist/search", get(search))
        .route(
            "/api/household-users/{owner_id}/watchlist/entries/{id}/metadata",
            axum::routing::post(enrich_entry),
        )
        .route("/api/watchlist/entries", get(entries).post(add_entry))
        .route(
            "/api/household-users/{owner_id}/watchlist/entries/{id}",
            get(entry).patch(edit_entry).delete(remove_entry),
        )
        .layer(Extension(provider))
}

async fn entries(
    State(state): State<AppState>,
    Query(filter): Query<EntriesFilter>,
) -> Result<Json<Vec<WatchlistEntry>>, WatchlistError> {
    Ok(Json(watchlist::entries(&state.db, filter).await?))
}

async fn add_entry(
    State(state): State<AppState>,
    Extension(provider): Extension<Option<TmdbService>>,
    Json(request): Json<AddEntry>,
) -> Result<(StatusCode, Json<Addition>), WatchlistError> {
    let addition = watchlist::add_entry(&state.db, request, provider.as_ref()).await?;
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

async fn edit_entry(
    State(state): State<AppState>,
    Path((owner_id, id)): Path<(String, String)>,
    Json(request): Json<EditEntry>,
) -> Result<Json<WatchlistEntry>, WatchlistError> {
    Ok(Json(
        watchlist::edit_entry(&state.db, &owner_id, &id, request).await?,
    ))
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
            Self::ProviderUnavailable => (StatusCode::SERVICE_UNAVAILABLE, Json(json!({
                "code": "tmdb_unavailable", "error": "TMDB is unavailable; you can still add a manual plan", "status": 503
            }))).into_response(),
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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Search {
    kind: MediaKind,
    query: String,
}

async fn search(
    Extension(provider): Extension<Option<TmdbService>>,
    Query(request): Query<Search>,
) -> Result<Json<Vec<MediaMetadata>>, WatchlistError> {
    let query = request.query.trim();
    if query.is_empty() || query.chars().count() > 200 || query.chars().any(char::is_control) {
        return Err(WatchlistError::Validation(
            "Search requires 1-200 characters without control characters".into(),
        ));
    }
    Ok(Json(
        provider
            .ok_or(WatchlistError::ProviderUnavailable)?
            .search(request.kind, query)
            .await?,
    ))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Enrichment {
    tmdb_id: i64,
}

async fn enrich_entry(
    State(state): State<AppState>,
    Extension(provider): Extension<Option<TmdbService>>,
    Path((owner_id, id)): Path<(String, String)>,
    Json(request): Json<Enrichment>,
) -> Result<Json<WatchlistEntry>, WatchlistError> {
    Ok(Json(
        watchlist::enrich_entry(
            &state.db,
            &owner_id,
            &id,
            request.tmdb_id,
            provider.as_ref(),
        )
        .await?,
    ))
}
