//! Isolated, loopback-only health, Wishlist, Watchlist, map and walking-loop seam for tests and
//! synthetic previews. Uses production routes, migrations and authentication without household
//! configuration; the map explicitly serves the unrelated Copenhagen demo origin.
//!
//! Walking loops come from a loopback provider serving a synthetic rectangle that is not a real
//! path. Set `OPENROUTESERVICE_API_KEY` to plan real loops from the demo origin instead.

use std::sync::Arc;

use axum::{Json, routing::post};
use openhome_api::{
    AppState, DockerCache, auth, routes,
    services::{
        map::MapOrigin,
        route_loop::{LoopPlanner, ProviderKey},
    },
};
use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let db = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await?;
    sqlx::migrate!("./migrations").run(&db).await?;
    let state = AppState {
        db,
        noop_db: SqlitePool::connect_lazy("sqlite::memory:")?,
        adguard_service: None,
        docker_service: None,
        ir_service: None,
        switchbot_service: None,
        docker_cache: Arc::new(tokio::sync::Mutex::new(DockerCache::default())),
    };
    let key = auth::ApiKey::new("synthetic-dashboard-api-key".to_string());
    let demo_origin = MapOrigin::parse(None, None, Some("true"))?;
    let planner =
        match ProviderKey::parse(std::env::var("OPENROUTESERVICE_API_KEY").ok().as_deref()) {
            Some(provider_key) => LoopPlanner::new(
                demo_origin,
                Some(provider_key),
                "https://api.openrouteservice.org/v2/directions/foot-walking/geojson".parse()?,
            )?,
            None => LoopPlanner::new(
                demo_origin,
                ProviderKey::parse(Some("synthetic-provider-key")),
                synthetic_provider().await?,
            )?,
        };
    let app = routes::health::router()
        .merge(routes::wishlist::router(None))
        .merge(routes::watchlist::router())
        .merge(routes::map::router(demo_origin))
        .merge(routes::route_loop::router(planner))
        .with_state(state)
        .layer(axum::middleware::from_fn(move |request, next| {
            auth::auth_middleware(request, next, key.clone())
        }));
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    println!("http://{}", listener.local_addr()?);
    axum::serve(listener, app).await?;
    Ok(())
}

/// Serve the synthetic demo loop in openrouteservice's response shape on loopback.
async fn synthetic_provider() -> anyhow::Result<url::Url> {
    let trip: serde_json::Value =
        serde_json::from_str(include_str!("../tests/fixtures/route-loop-copenhagen.json"))?;
    let path = "/v2/directions/foot-walking/geojson";
    let provider = axum::Router::new().route(path, post(move || async move { Json(trip) }));
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let url = format!("http://{}{path}", listener.local_addr()?).parse()?;
    tokio::spawn(async move { axum::serve(listener, provider).await });
    Ok(url)
}
