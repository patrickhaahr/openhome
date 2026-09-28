use rmcp::transport::streamable_http_server::StreamableHttpServerConfig;
use tokio::net::TcpListener;
use tokio::signal;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use openhome_mcp::{Config, MCP_PATH, OpenHomeApi};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::from_env()?;
    let http_config =
        StreamableHttpServerConfig::default().with_allowed_hosts(config.allowed_hosts.clone());
    // Cancelling ends open MCP sessions, which would otherwise hold graceful shutdown open.
    let sessions = http_config.cancellation_token.clone();
    let api = OpenHomeApi::new(config.api_url.clone(), config.api_key.clone())?;
    let app = openhome_mcp::router(api, config.api_key.clone(), http_config);

    let listener = TcpListener::bind(config.bind_addr).await?;
    tracing::info!(
        address = %listener.local_addr()?,
        path = MCP_PATH,
        api = %config.api_url,
        allowed_hosts = ?config.allowed_hosts,
        "OpenHome MCP adapter listening"
    );
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            shutdown_signal().await;
            sessions.cancel();
        })
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
        () = ctrl_c => tracing::info!("Received Ctrl+C, shutting down"),
        () = terminate => tracing::info!("Received SIGTERM, shutting down"),
    }
}
