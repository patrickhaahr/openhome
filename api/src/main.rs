use axum::Router;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::signal;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use openhome_api::auth;
use openhome_api::routes;
use openhome_api::services::{
    adguard, backlight, docker, feed, ir, map, noop_push, route_loop, switchbot, weather,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let database_url =
        std::env::var("DATABASE_URL").expect("DATABASE_URL environment variable must be set");
    let db = openhome_api::db::connect_sqlite(&database_url).await?;

    sqlx::migrate!("./migrations").run(&db).await?;

    let noop_db_url =
        noop_push::resolve_db_url(&database_url, std::env::var("NOOP_DB_URL").ok().as_deref())?;
    let noop_db = noop_push::connect(&noop_db_url).await?;

    let adguard_host = std::env::var("ADGUARD_HOST").unwrap_or_default();
    let adguard_username = std::env::var("ADGUARD_USERNAME").unwrap_or_default();
    let adguard_password = std::env::var("ADGUARD_PASSWORD").unwrap_or_default();
    let adguard_insecure_tls = std::env::var("ADGUARD_INSECURE_TLS").unwrap_or_default() == "true";
    let ir_base_url = std::env::var("IR_BASE_URL").unwrap_or_default();
    let switchbot_base_url = std::env::var("SWITCHBOT_BASE_URL").unwrap_or_default();
    let weather_service = weather::WeatherLocation::parse(
        non_empty_env("WEATHER_LATITUDE").as_deref(),
        non_empty_env("WEATHER_LONGITUDE").as_deref(),
    )?
    .map(|location| {
        weather::WeatherService::new(
            location,
            url::Url::parse("https://api.open-meteo.com/v1/forecast")?,
        )
    })
    .transpose()?;

    let photo_service = openhome_api::services::photos::PhotoService::configure(
        non_empty_env("IMMICH_URL").as_deref(),
        non_empty_env("IMMICH_API_KEY").as_deref(),
        non_empty_env("IMMICH_ALBUM_IDS").as_deref(),
    )?;

    let map_origin = map::MapOrigin::parse(
        non_empty_env("MAP_ORIGIN_LATITUDE").as_deref(),
        non_empty_env("MAP_ORIGIN_LONGITUDE").as_deref(),
        non_empty_env("MAP_DEMO_ORIGIN").as_deref(),
    )?;
    // Loops start at the shared map origin, so routes draw where the map opens.
    let loop_planner = route_loop::LoopPlanner::new(
        map_origin,
        route_loop::ProviderEndpoint::parse(non_empty_env("OPENROUTESERVICE_URL").as_deref())?,
    )?;

    let docker_service = if std::path::Path::new("/var/run/docker.sock").exists() {
        match docker::DockerService::new() {
            Ok(service) => {
                tracing::info!("Docker service initialized successfully");
                Some(service)
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to initialize Docker service");
                None
            }
        }
    } else {
        tracing::warn!(
            "Docker socket not found at /var/run/docker.sock, Docker integration disabled"
        );
        None
    };

    let adguard_service = if adguard_host.is_empty() {
        tracing::warn!("ADGUARD_HOST not set, AdGuard integration disabled");
        None
    } else {
        Some(adguard::AdguardService::new(
            &adguard_host,
            &adguard_username,
            &adguard_password,
            adguard_insecure_tls,
        )?)
    };

    let ir_service = if ir_base_url.is_empty() {
        tracing::warn!("IR_BASE_URL not set, IR integration disabled");
        None
    } else {
        Some(ir::IrService::new(&ir_base_url)?)
    };

    let switchbot_service = if switchbot_base_url.is_empty() {
        tracing::warn!("SWITCHBOT_BASE_URL not set, SwitchBot integration disabled");
        None
    } else {
        Some(switchbot::SwitchbotService::new(&switchbot_base_url)?)
    };

    let backlight_transport = non_empty_env("BACKLIGHT_BLUETOOTH_ADDRESS")
        .map(|address| backlight::Bluetooth::configure(&address).map_err(anyhow::Error::msg))
        .transpose()?;
    let backlight_service = backlight::BacklightService::new(
        db.clone(),
        backlight_transport,
        non_empty_env("BACKLIGHT_PROFILE_BACKUP").map(std::path::PathBuf::from),
    )
    .await?;

    let state = openhome_api::AppState {
        db,
        noop_db: noop_db.clone(),
        adguard_service,
        docker_service,
        ir_service,
        switchbot_service,
        docker_cache: std::sync::Arc::new(tokio::sync::Mutex::new(
            openhome_api::DockerCache::default(),
        )),
    };

    let api_key = auth::ApiKey::new(
        std::env::var("API_KEY").expect("API_KEY environment variable must be set"),
    );
    let api_key_clone = api_key.clone();

    let push_token = noop_push::PushToken::new(
        std::env::var("NOOP_PUSH_TOKEN").expect("NOOP_PUSH_TOKEN environment variable must be set"),
    )?;
    if push_token.matches(api_key.as_str()) {
        anyhow::bail!("NOOP_PUSH_TOKEN must differ from API_KEY");
    }
    let push_state = routes::noop_push::PushState {
        db: noop_db,
        token: push_token,
    };
    tracing::info!(
        path = routes::noop_push::PUSH_PATH,
        "NOOP push endpoint enabled"
    );

    let app = Router::new()
        .merge(routes::health::router())
        .merge(routes::feeds::router())
        .merge(routes::fitness::router())
        .merge(routes::timeline::router())
        .merge(routes::adguard::router())
        .merge(routes::docker::router())
        .merge(routes::ir::router())
        .merge(routes::map::router(map_origin))
        .merge(routes::route_loop::router(loop_planner))
        .merge(routes::switchbot::router())
        .merge(routes::training::router())
        .merge(routes::watchlist::router(
            non_empty_env("TMDB_READ_ACCESS_TOKEN").and_then(|token| {
                let base_url = url::Url::parse("https://api.themoviedb.org/3/").ok()?;
                if let Ok(service) =
                    openhome_api::services::tmdb::TmdbService::new(&token, base_url)
                {
                    Some(service)
                } else {
                    tracing::warn!(
                        "TMDB configuration is invalid; manual watchlists remain available"
                    );
                    None
                }
            }),
        ))
        .merge(routes::wishlist::router(
            std::env::var("WISHLIST_DEFAULT_LIST_ID")
                .ok()
                .filter(|id| !id.trim().is_empty()),
        ))
        .merge(routes::important_dates::router())
        .merge(routes::weather::router(weather_service))
        .merge(routes::backlight::router(backlight_service))
        .merge(routes::photos::router(photo_service))
        .with_state(state.clone())
        .layer(axum::middleware::from_fn(move |req, next| {
            openhome_api::auth::auth_middleware(req, next, api_key_clone.clone())
        }))
        // Merged after the API-key layer: the push route authenticates with NOOP_PUSH_TOKEN.
        .merge(routes::noop_push::router(push_state))
        .layer(TraceLayer::new_for_http());

    let scheduler_state = state.clone();
    tokio::spawn(async move {
        tracing::info!("Starting RSS feed scheduler (initial fetch + 24h interval)");
        if let Err(e) = feed::refresh_all_feeds(&scheduler_state.db).await {
            tracing::warn!(error = %e, "Initial RSS feed refresh failed");
        }
        loop {
            tokio::time::sleep(Duration::from_hours(24)).await;
            tracing::info!("Running scheduled RSS feed refresh");
            if let Err(e) = feed::refresh_all_feeds(&scheduler_state.db).await {
                tracing::warn!(error = %e, "Scheduled RSS feed refresh failed");
                tokio::time::sleep(Duration::from_secs(60)).await;
            }
        }
    });

    let listener = TcpListener::bind("0.0.0.0:8000").await?;
    tracing::info!("Listening on {}", listener.local_addr()?);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

fn non_empty_env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {
            tracing::info!("Received Ctrl+C, starting graceful shutdown");
        }
        () = terminate => {
            tracing::info!("Received SIGTERM, starting graceful shutdown");
        }
    }
}
