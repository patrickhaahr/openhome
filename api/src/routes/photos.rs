//! Authenticated gallery metadata and resized media, scoped by the integration service.
use crate::services::photos::{PhotoError, PhotoService, PhotoSize};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

/// Merge inside the API-key layer, as with every private `OpenHome` read.
pub fn router(service: Option<PhotoService>) -> Router<crate::AppState> {
    Router::new()
        .route("/api/photos/albums", get(list))
        .route("/api/photos/all", get(merged))
        .route("/api/photos/albums/{album}", get(page))
        .route(
            "/api/photos/albums/{album}/assets/{asset}/{size}",
            get(image),
        )
        .with_state(service)
        .layer(axum::middleware::from_fn(
            |request, next: axum::middleware::Next| async move {
                let mut response = next.run(request).await;
                response.headers_mut().insert(
                    header::CACHE_CONTROL,
                    header::HeaderValue::from_static("no-store"),
                );
                response.headers_mut().insert(
                    "x-content-type-options",
                    header::HeaderValue::from_static("nosniff"),
                );
                response
            },
        ))
}
fn configured(service: Option<&PhotoService>) -> Result<&PhotoService, PhotoError> {
    service.ok_or(PhotoError::MissingConfiguration)
}
async fn list(
    State(service): State<Option<PhotoService>>,
) -> Result<Json<serde_json::Value>, PhotoError> {
    Ok(Json(
        json!({"state":"ready", "albums": configured(service.as_ref())?.list().await?}),
    ))
}
#[derive(Deserialize, Default)]
struct PageQuery {
    #[serde(default)]
    page: usize,
}
async fn page(
    State(service): State<Option<PhotoService>>,
    Path(album): Path<Uuid>,
    Query(query): Query<PageQuery>,
) -> Result<Json<serde_json::Value>, PhotoError> {
    Ok(Json(
        json!({"state":"ready", "photos": configured(service.as_ref())?.page(album, query.page).await?}),
    ))
}
async fn merged(
    State(service): State<Option<PhotoService>>,
    Query(query): Query<PageQuery>,
) -> Result<Json<serde_json::Value>, PhotoError> {
    Ok(Json(
        json!({"state":"ready", "photos": configured(service.as_ref())?.merged(query.page).await?}),
    ))
}
async fn image(
    State(service): State<Option<PhotoService>>,
    Path((album, asset, size)): Path<(Uuid, Uuid, PhotoSize)>,
) -> Result<Response, PhotoError> {
    let (content_type, body) = configured(service.as_ref())?
        .image(album, asset, size)
        .await?;
    Ok(([(header::CONTENT_TYPE, content_type)], body).into_response())
}
impl IntoResponse for PhotoError {
    fn into_response(self) -> Response {
        let (status, reason) = match self {
            Self::MissingConfiguration => {
                (StatusCode::SERVICE_UNAVAILABLE, "missing-configuration")
            }
            Self::NotFound => (StatusCode::NOT_FOUND, "not-found"),
            Self::ProviderUnavailable => (StatusCode::SERVICE_UNAVAILABLE, "provider-unavailable"),
            Self::InvalidPage => (StatusCode::BAD_REQUEST, "invalid-page"),
        };
        (
            status,
            Json(json!({"state":"unavailable", "reason":reason})),
        )
            .into_response()
    }
}
