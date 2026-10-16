//! One-command synthetic preview of the whole dashboard for UI design work.
//!
// A design fixture, not production code: lossy numeric casts and short names are deliberate.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::too_many_lines,
    clippy::trivially_copy_pass_by_ref
)]

//! Serves the production routes, migrations and authentication over isolated in-memory databases,
//! with loopback stand-ins for every upstream (openrouteservice, TMDB, `Open-Meteo`, Immich, the IR
//! bridge and the `SwitchBot` bridge). All data is invented and readable in `seed/*.json`; it is
//! generated relative to today so the current day is always populated. The household API,
//! credentials and coordinates are never touched: the map and weather use the public Copenhagen
//! demo origin.
//!
//! Run through `just dashboard-synthetic`. Edit `seed/` and restart to change what the preview shows.

use std::sync::Arc;

use openhome_api::{
    AppState, DockerCache, auth, routes,
    services::{
        ir::IrService,
        map::MapOrigin,
        noop_push,
        photos::PhotoService,
        route_loop::LoopPlanner,
        switchbot::SwitchbotService,
        weather::{WeatherLocation, WeatherService},
    },
};
use sqlx::sqlite::SqlitePoolOptions;
use tokio::net::TcpListener;

#[path = "../support/providers.rs"]
mod providers;

mod noop;
mod seed;
mod stubs;

/// Not secret: it only authorizes this loopback fixture.
const API_KEY: &str = "synthetic-dashboard-api-key";
const DEFAULT_PORT: &str = "3101";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let db = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await?;
    sqlx::migrate!("./migrations").run(&db).await?;
    let noop_db = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await?;
    noop_push::initialize(&noop_db).await?;
    noop::seed(&noop_db).await?;

    let ir = IrService::new(&stubs::ir_bridge().await?)?;
    let switchbot = SwitchbotService::new(&stubs::switchbot_bridge().await?)?;
    let weather = WeatherService::new(
        WeatherLocation::parse(Some("55.6869"), Some("12.5692"))?
            .ok_or_else(|| anyhow::anyhow!("demo location is required"))?,
        stubs::open_meteo().await?,
    )?;
    let photos = PhotoService::new(
        stubs::immich().await?,
        "synthetic-photo-key",
        stubs::album_ids(),
    )?;
    let demo_origin = MapOrigin::parse(None, None, Some("true"))?;
    let planner = LoopPlanner::new(demo_origin, Some(providers::synthetic_provider().await?))?;
    let tmdb = providers::synthetic_tmdb().await?;

    let state = AppState {
        db,
        noop_db,
        adguard_service: None,
        docker_service: None,
        ir_service: Some(ir),
        switchbot_service: Some(switchbot),
        docker_cache: Arc::new(tokio::sync::Mutex::new(DockerCache::default())),
    };
    let key = auth::ApiKey::new(API_KEY.to_string());
    let app = routes::health::router()
        .merge(routes::wishlist::router(None))
        .merge(routes::watchlist::router(Some(tmdb)))
        .merge(routes::map::router(demo_origin))
        .merge(routes::route_loop::router(planner))
        .merge(routes::photos::router(Some(photos)))
        .merge(routes::weather::router(Some(weather)))
        .merge(routes::ir::router())
        .merge(routes::switchbot::router())
        .merge(routes::fitness::router())
        .merge(routes::training::router())
        .with_state(state)
        .layer(axum::middleware::from_fn(move |request, next| {
            auth::auth_middleware(request, next, key.clone())
        }));
    let port = std::env::var("OPENHOME_PREVIEW_API_PORT").unwrap_or_else(|_| DEFAULT_PORT.into());
    let listener = TcpListener::bind(format!("127.0.0.1:{port}")).await?;
    let url = format!("http://{}", listener.local_addr()?);
    tokio::spawn(async move { axum::serve(listener, app).await });

    // Household records are seeded through the public routes, so they always match the contract.
    seed::household(&url, API_KEY).await?;
    println!("{url}");
    tokio::signal::ctrl_c().await?;
    Ok(())
}
