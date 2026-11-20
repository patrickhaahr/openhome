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

use std::sync::Arc;

use openhome_api::{
    AppState, DockerCache, auth, routes,
    services::{
        exchange_rates,
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
#[allow(dead_code)]
mod providers;

#[path = "../support/foods.rs"]
mod foods;

mod binder;
mod ecb;
mod media;
mod noop;
mod photos;
mod prices;
mod seed;
mod stubs;

const API_KEY: &str = "synthetic-dashboard-api-key";
const DEFAULT_PORT: &str = "3101";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let db = SqlitePoolOptions::new()
        .max_connections(1)
        .idle_timeout(None)
        .max_lifetime(None)
        .connect("sqlite::memory:")
        .await?;
    sqlx::migrate!("./migrations").run(&db).await?;
    let noop_db = SqlitePoolOptions::new()
        .max_connections(1)
        .idle_timeout(None)
        .max_lifetime(None)
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
        photos::immich().await?,
        "synthetic-photo-key",
        photos::album_ids(),
    )?;
    let demo_origin = MapOrigin::parse(None, None, Some("true"))?;
    let planner = LoopPlanner::new(demo_origin, Some(providers::synthetic_provider().await?))?;
    let tmdb = media::tmdb().await?;
    let catalogue = binder::stand_in().await?;
    let optcg = catalogue.client.clone();
    let mode = ecb::mode();
    let ecb = ecb::ecb(mode).await?;
    if mode == ecb::Mode::MissedDay {
        tokio::spawn(ecb::next_pass(db.clone(), ecb.clone()));
    }
    tokio::spawn(exchange_rates::run_scheduler(db.clone(), ecb));
    let food_state = foods::FoodState::from_args();
    let foods = foods::synthetic_foods(food_state).await?;
    let (meal_photos, _meal_photo_dir) = foods::photo_store()?;
    let labels = providers::synthetic_open_food_facts().await?;
    if food_state == foods::FoodState::Ready {
        foods::seed_meals(&db, &foods, &meal_photos, &labels).await?;
    }

    let seed_db = db.clone();
    let picture_db = db.clone();
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
        .merge(routes::cards::router(optcg.clone()))
        .merge(routes::binder::router(optcg))
        .merge(routes::important_dates::router())
        .merge(routes::map::router(demo_origin))
        .merge(routes::route_loop::router(planner))
        .merge(routes::photos::router(Some(photos)))
        .merge(routes::weather::router(Some(weather)))
        .merge(routes::ir::router())
        .merge(routes::switchbot::router())
        .merge(routes::fitness::router())
        .merge(routes::training::router())
        .merge(routes::nutrition::router(foods, meal_photos, labels))
        .with_state(state)
        .layer(axum::middleware::from_fn(move |request, next| {
            media::wish_picture(picture_db.clone(), request, next)
        }))
        .layer(axum::middleware::from_fn(move |request, next| {
            auth::auth_middleware(request, next, key.clone())
        }));
    let port = std::env::var("OPENHOME_PREVIEW_API_PORT").unwrap_or_else(|_| DEFAULT_PORT.into());
    let listener = TcpListener::bind(format!("127.0.0.1:{port}")).await?;
    let url = format!("http://{}", listener.local_addr()?);
    tokio::spawn(async move { axum::serve(listener, app).await });

    seed::household(&url, API_KEY, &seed_db).await?;
    if binder::history_mode() {
        prices::replay(&seed_db, &catalogue).await?;
        tokio::spawn(prices::restarts(seed_db.clone(), catalogue.client.clone()));
    }
    println!("{url}");
    tokio::signal::ctrl_c().await?;
    Ok(())
}
