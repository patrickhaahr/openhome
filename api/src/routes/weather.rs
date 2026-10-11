use axum::{Json, Router, extract::State, routing::get};

use crate::services::weather::{WeatherRead, WeatherService, WeatherUnavailable};

/// The weather capability owns its shared cache; it needs no database state.
pub fn router(service: Option<WeatherService>) -> Router<crate::AppState> {
    Router::new()
        .route("/api/weather", get(read))
        .with_state(service)
}

async fn read(State(service): State<Option<WeatherService>>) -> Json<WeatherRead> {
    Json(match service {
        Some(service) => service.read().await,
        None => WeatherRead::Unavailable {
            reason: WeatherUnavailable::MissingConfiguration,
        },
    })
}
