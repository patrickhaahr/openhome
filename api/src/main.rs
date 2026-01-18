use axum::Router;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::str::FromStr;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::signal;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use rpi_api::auth;
use rpi_api::services::{adguard, feed};

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
    let options = SqliteConnectOptions::from_str(&database_url)?
        .create_if_missing(true)
        .foreign_keys(true)
        .busy_timeout(Duration::from_secs(5));
    let db = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;

    sqlx::migrate!("./migrations").run(&db).await?;

    let adguard_host = std::env::var("ADGUARD_HOST").unwrap_or_default();
    let adguard_username = std::env::var("ADGUARD_USERNAME").unwrap_or_default();
    let adguard_password = std::env::var("ADGUARD_PASSWORD").unwrap_or_default();
    let adguard_insecure_tls = std::env::var("ADGUARD_INSECURE_TLS").unwrap_or_default() == "true";

    let adguard_service = if !adguard_host.is_empty() {
        Some(adguard::AdguardService::new(
            &adguard_host,
            &adguard_username,
            &adguard_password,
            adguard_insecure_tls,
        )?)
    } else {
        tracing::warn!("ADGUARD_HOST not set, AdGuard integration disabled");
        None
    };

    let state = rpi_api::AppState {
        db,
        adguard_service,
    };

    let api_key = auth::ApiKey::new(
        std::env::var("API_KEY").expect("API_KEY environment variable must be set"),
    );
    let api_key_clone = api_key.clone();

    let app = Router::new()
        .merge(rpi_api::routes::health::router())
        .merge(rpi_api::routes::facts::router())
        .merge(rpi_api::routes::feeds::router())
        .merge(rpi_api::routes::timeline::router())
        .merge(rpi_api::routes::adguard::router())
        .with_state(state.clone())
        .layer(axum::middleware::from_fn(move |req, next| {
            rpi_api::auth::auth_middleware(req, next, api_key_clone.clone())
        }))
        .layer(TraceLayer::new_for_http());

    let scheduler_state = state.clone();
    tokio::spawn(async move {
        tracing::info!("Starting RSS feed scheduler (initial fetch + 24h interval)");
        if let Err(e) = feed::refresh_all_feeds(&scheduler_state.db).await {
            tracing::warn!(error = %e, "Initial RSS feed refresh failed");
        }
        loop {
            tokio::time::sleep(Duration::from_secs(24 * 60 * 60)).await;
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
        _ = ctrl_c => {
            tracing::info!("Received Ctrl+C, starting graceful shutdown");
        }
        _ = terminate => {
            tracing::info!("Received SIGTERM, starting graceful shutdown");
        }
    }
}
