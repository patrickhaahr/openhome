use axum::Router;
use tokio::net::TcpListener;
use tokio::signal;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod auth;
mod error;
mod routes;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    dotenvy::dotenv().ok();

    let api_key = auth::ApiKey::new(
        std::env::var("API_KEY").expect("API_KEY environment variable must be set"),
    );
    let api_key_clone = api_key.clone();

    let app = Router::new()
        .merge(routes::health::router())
        .merge(routes::facts::router())
        .layer(axum::middleware::from_fn(move |req, next| {
            auth::auth_middleware(req, next, api_key_clone.clone())
        }))
        .layer(TraceLayer::new_for_http());

    let listener = TcpListener::bind("0.0.0.0:8000").await.unwrap();
    tracing::info!("Listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();
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
