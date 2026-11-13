use axum::{
    Extension, Json, Router,
    extract::{Path, Query, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use serde::Deserialize;
use serde_json::json;

use crate::{
    AppState,
    error::AppError,
    services::{
        cards::{self, CardCode, CardId, CardsError, Search},
        optcg::Optcg,
        price_history::{self, PriceHistory},
    },
};

pub fn router(catalogue: Optcg) -> Router<AppState> {
    Router::new()
        .route("/api/binder/cards", get(search))
        .route("/api/binder/cards/{id}/image", get(image))
        .route("/api/binder/cards/{id}/prices", get(prices))
        .layer(Extension(catalogue))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SearchQuery {
    card_code: String,
}

async fn search(
    State(state): State<AppState>,
    Extension(catalogue): Extension<Optcg>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<Search>, CardsError> {
    let code = CardCode::parse(&query.card_code)?;
    Ok(Json(cards::search(&state.db, &catalogue, &code).await?))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PricesQuery {
    from: Option<String>,
    to: Option<String>,
}

async fn prices(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(range): Query<PricesQuery>,
) -> Result<Json<PriceHistory>, CardsError> {
    Ok(Json(
        price_history::history(
            &state.db,
            &CardId::parse(&id)?,
            range.from.as_deref(),
            range.to.as_deref(),
        )
        .await?,
    ))
}

async fn image(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, CardsError> {
    let avif = cards::image(&state.db, &CardId::parse(&id)?).await?;
    Ok((
        [
            (header::CONTENT_TYPE, "image/avif"),
            // The stored image never changes, but it is household data: no shared caches.
            (header::CACHE_CONTROL, "private, max-age=86400"),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
        ],
        avif,
    )
        .into_response())
}

impl IntoResponse for CardsError {
    fn into_response(self) -> Response {
        match self {
            Self::DataUnavailable => (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({
                    "code": "card_data_unavailable",
                    "error": "The card catalogue is unavailable; try again later",
                    "status": 503,
                })),
            )
                .into_response(),
            Self::Validation(message) => AppError::Validation(message).into_response(),
            Self::NotFound(message) => AppError::NotFound(message).into_response(),
            Self::Encoder => {
                AppError::Internal(anyhow::anyhow!("card image encoding failed")).into_response()
            }
            Self::Database(error) => AppError::Internal(error.into()).into_response(),
        }
    }
}
