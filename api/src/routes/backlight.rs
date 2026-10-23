use crate::{
    error::Result,
    services::backlight::{BacklightService, ProfileSource, Selection, Status},
};
use axum::{
    Json, Router,
    extract::State,
    routing::{get, post, put},
};

pub fn router(service: BacklightService) -> Router<crate::AppState> {
    Router::new()
        .route("/api/backlight", get(read))
        .route("/api/backlight/selection", put(select))
        .route("/api/backlight/profile", post(profile))
        .route("/api/backlight/refresh", post(refresh))
        .with_state(service)
}
async fn read(State(service): State<BacklightService>) -> Json<Status> {
    Json(service.status().await)
}
async fn select(
    State(service): State<BacklightService>,
    Json(selection): Json<Selection>,
) -> Result<Json<Status>> {
    service.select(selection).await.map(Json)
}

async fn profile(
    State(service): State<BacklightService>,
    Json(source): Json<ProfileSource>,
) -> Result<Json<Status>> {
    service.save_profile(source).await.map(Json)
}
async fn refresh(State(service): State<BacklightService>) -> Json<Status> {
    Json(service.refresh().await)
}
