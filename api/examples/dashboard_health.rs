//! Isolated, loopback-only health seam for dashboard tests and synthetic previews.
//! Uses the production health route and authentication without household configuration.

use std::sync::Arc;

use openhome_api::{AppState, DockerCache, auth, routes};
use sqlx::SqlitePool;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let state = AppState {
        db: SqlitePool::connect_lazy("sqlite::memory:")?,
        noop_db: SqlitePool::connect_lazy("sqlite::memory:")?,
        adguard_service: None,
        docker_service: None,
        ir_service: None,
        switchbot_service: None,
        docker_cache: Arc::new(tokio::sync::Mutex::new(DockerCache::default())),
    };
    let key = auth::ApiKey::new("synthetic-dashboard-api-key".to_string());
    let app = routes::health::router()
        .with_state(state)
        .layer(axum::middleware::from_fn(move |request, next| {
            auth::auth_middleware(request, next, key.clone())
        }));
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    println!("http://{}", listener.local_addr()?);
    axum::serve(listener, app).await?;
    Ok(())
}
