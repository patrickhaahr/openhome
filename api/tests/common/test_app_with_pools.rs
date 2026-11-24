use axum::Router;
use openhome_api::AppState;
use openhome_api::auth::{ApiKey, auth_middleware};
use openhome_api::routes::{
    adguard::router as adguard_router, feeds::router as feeds_router,
    fitness::router as fitness_router, health::router as health_router, ir::router as ir_router,
    switchbot::router as switchbot_router, timeline::router as timeline_router,
    training::router as training_router,
};
use sqlx::SqlitePool;

pub fn test_app_with_pools(db: SqlitePool, noop_db: SqlitePool) -> (Router, AppState) {
    let api_key = ApiKey::new("test-api-key".to_string());
    let api_key_clone = api_key;

    let state = AppState {
        db,
        noop_db,
        adguard_service: None,
        docker_service: None,
        ir_service: None,
        switchbot_service: None,
        docker_cache: std::sync::Arc::new(tokio::sync::Mutex::new(
            openhome_api::DockerCache::default(),
        )),
    };

    let app = health_router()
        .merge(adguard_router())
        .merge(feeds_router())
        .merge(fitness_router())
        .merge(ir_router())
        .merge(switchbot_router())
        .merge(timeline_router())
        .merge(training_router())
        .merge(openhome_api::routes::wishlist::router(None))
        .merge(openhome_api::routes::watchlist::router(None))
        .merge(openhome_api::routes::important_dates::router())
        .with_state(state.clone())
        .layer(axum::middleware::from_fn(move |req, next| {
            auth_middleware(req, next, api_key_clone.clone())
        }));

    (app, state)
}
