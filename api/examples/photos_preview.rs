//! Explicitly enabled loopback preview of approved demo albums; never connects a household database.
use openhome_api::{
    AppState, DockerCache, auth, routes,
    services::photos::{PhotoService, PhotoSize},
};
use sqlx::SqlitePool;
use std::sync::Arc;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    if std::env::var("IMMICH_PUBLIC_DEMO").ok().as_deref() != Some("true") {
        anyhow::bail!("Set IMMICH_PUBLIC_DEMO=true only with albums approved for public evidence");
    }
    let service = PhotoService::configure(
        std::env::var("IMMICH_URL").ok().as_deref(),
        std::env::var("IMMICH_API_KEY").ok().as_deref(),
        std::env::var("IMMICH_ALBUM_IDS").ok().as_deref(),
    )?
    .ok_or_else(|| anyhow::anyhow!("Photo preview configuration required"))?;
    let albums = service.list().await?;
    let mut photos = 0;
    for album in albums {
        let page = service.page(album.id, 0).await?;
        for id in page.assets {
            for size in [PhotoSize::Thumbnail, PhotoSize::Preview] {
                service.image(album.id, id, size).await?;
            }
            photos += 1;
        }
    }
    println!(
        "Installed-provider smoke passed: {photos} photos, thumbnail and preview; originals never requested"
    );
    let db = SqlitePool::connect_lazy("sqlite::memory:")?;
    let state = AppState {
        db: db.clone(),
        noop_db: db,
        adguard_service: None,
        docker_service: None,
        ir_service: None,
        switchbot_service: None,
        docker_cache: Arc::new(tokio::sync::Mutex::new(DockerCache::default())),
    };
    let key = auth::ApiKey::new("synthetic-dashboard-api-key".into());
    let app = routes::health::router()
        .merge(routes::photos::router(Some(service)))
        .with_state(state)
        .layer(axum::middleware::from_fn(move |req, next| {
            auth::auth_middleware(req, next, key.clone())
        }));
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    println!("http://{}", listener.local_addr()?);
    axum::serve(listener, app).await?;
    Ok(())
}
