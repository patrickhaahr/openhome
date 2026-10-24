use crate::{
    error::Result,
    services::backlight::{
        BacklightService, InactivitySettings, ProfileSource, Selection, Status,
        routine::BedtimeUpdate,
    },
};
use axum::{
    Json, Router,
    extract::State,
    routing::{get, patch, post, put},
};

pub fn router(service: BacklightService) -> Router<crate::AppState> {
    Router::new()
        .route("/api/backlight", get(read))
        .route("/api/backlight/selection", put(select))
        .route("/api/backlight/automatic", post(automatic))
        .route("/api/backlight/bedtime", patch(bedtime))
        .route("/api/backlight/profile", post(profile))
        .route("/api/backlight/refresh", post(refresh))
        .route(
            "/api/backlight/inactivity",
            get(inactivity).post(set_inactivity),
        )
        .with_state(service)
}
async fn inactivity(State(service): State<BacklightService>) -> Result<Json<InactivitySettings>> {
    service.inactivity(None).await.map(Json)
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct InactivityUpdate {
    seconds: u16,
}
async fn set_inactivity(
    State(service): State<BacklightService>,
    Json(update): Json<InactivityUpdate>,
) -> Result<Json<InactivitySettings>> {
    service.inactivity(Some(update.seconds)).await.map(Json)
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

async fn bedtime(
    State(service): State<BacklightService>,
    Json(update): Json<BedtimeUpdate>,
) -> Result<Json<Status>> {
    service.update_bedtime(update).await.map(Json)
}
async fn automatic(State(service): State<BacklightService>) -> Result<Json<Status>> {
    service.automatic().await.map(Json)
}
