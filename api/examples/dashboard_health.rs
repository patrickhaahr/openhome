//! Isolated, loopback-only health, Wishlist, Watchlist and map seam for tests and synthetic previews.
//! Uses production routes, migrations and authentication without household configuration;
//! the map explicitly serves the unrelated Copenhagen demo origin.

use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, Query},
    http::StatusCode,
    routing::get,
};
use openhome_api::services::tmdb::TmdbService;
use openhome_api::{AppState, DockerCache, auth, routes, services::map::MapOrigin};
use serde::Deserialize;
use serde_json::{Value, json};
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
    let provider = axum::Router::new()
        .route("/3/search/{kind}", get(provider_search))
        .route("/3/{kind}/{id}", get(provider_details));
    let provider_listener = TcpListener::bind("127.0.0.1:0").await?;
    let provider_url = format!("http://{}/3/", provider_listener.local_addr()?);
    tokio::spawn(async move { axum::serve(provider_listener, provider).await });
    let tmdb = TmdbService::new("synthetic-provider-token", provider_url.parse()?)?;
    let app = routes::health::router()
        .merge(routes::wishlist::router(None))
        .merge(routes::watchlist::router(Some(tmdb)))
        .merge(routes::map::router(demo_origin))
        .merge(routes::photos::router(None))
        .with_state(state)
        .layer(axum::middleware::from_fn(move |request, next| {
            auth::auth_middleware(request, next, key.clone())
        }));
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    println!("http://{}", listener.local_addr()?);
    axum::serve(listener, app).await?;
    Ok(())
}

// Synthetic upstream exercises the real private adapter without a provider token or household data.
fn provider_title(kind: &str, id: i64) -> Value {
    let mut item = json!({ "id": id, "overview": "A fictional journey through distant skies, with an unexpected friendship along the way.", "poster_path": "/synthetic.jpg" });
    if kind == "movie" {
        item["title"] = json!("A Quiet Orbit");
        item["release_date"] = json!("2020-02-29");
    } else {
        item["name"] = json!("The Lighthouse Journey");
        item["first_air_date"] = json!("2021-01-01");
    }
    item
}

#[derive(Deserialize)]
struct ProviderSearch {
    query: String,
}

async fn provider_search(
    Path(kind): Path<String>,
    Query(search): Query<ProviderSearch>,
) -> Result<Json<Value>, StatusCode> {
    if search.query.to_lowercase().contains("offline") {
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    }
    if search.query == "Replacement poster" {
        return Ok(Json(json!({ "results": [provider_title(&kind, 44)] })));
    }
    if search.query == "Unavailable details" {
        return Ok(Json(json!({ "results": [provider_title(&kind, 43)] })));
    }
    let title = provider_title(&kind, 42);
    let name = if kind == "movie" {
        "A Quiet Orbit"
    } else {
        "The Lighthouse Journey"
    };
    let results = if name.to_lowercase().contains(&search.query.to_lowercase()) {
        vec![title]
    } else {
        vec![]
    };
    Ok(Json(json!({ "results": results })))
}

async fn provider_details(
    Path((kind, id)): Path<(String, i64)>,
) -> Result<Json<Value>, StatusCode> {
    if id == 44 {
        let mut item = provider_title(&kind, id);
        item["poster_path"] = json!("/replacement.jpg");
        return Ok(Json(item));
    }
    if id != 42 {
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    }
    Ok(Json(provider_title(&kind, id)))
}
