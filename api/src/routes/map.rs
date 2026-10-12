use axum::{Json, Router, extract::State, routing::get};

use crate::services::map::{MapOrigin, MapOriginRead};

/// The map capability reads its validated startup origin; it needs no database state.
pub fn router(origin: Option<MapOrigin>) -> Router<crate::AppState> {
    Router::new()
        .route("/api/map", get(read))
        .with_state(origin)
}

async fn read(State(origin): State<Option<MapOrigin>>) -> Json<MapOriginRead> {
    Json(origin.into())
}
