//! Isolated, loopback-only health, Wishlist, Watchlist, fitness, map and walking-loop seam for
//! tests and synthetic previews. Uses production routes, migrations and authentication without
//! household configuration; the map explicitly serves the unrelated Copenhagen demo origin.
//!
//! Loops come from a loopback provider serving synthetic squares that are not real paths. Set
//! `OPENROUTESERVICE_URL` to a household instance to plan real loops from the demo origin instead.

use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, Query},
    http::StatusCode,
    routing::{get, post},
};
use openhome_api::{
    AppState, DockerCache, auth, routes,
    services::{
        map::MapOrigin,
        route_loop::{LoopPlanner, ProviderEndpoint},
        tmdb::TmdbService,
    },
};
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
    // A configured household instance plans real loops; otherwise serve synthetic ones.
    let endpoint =
        match ProviderEndpoint::parse(std::env::var("OPENROUTESERVICE_URL").ok().as_deref())? {
            Some(endpoint) => endpoint,
            None => synthetic_provider().await?,
        };
    let planner = LoopPlanner::new(demo_origin, Some(endpoint))?;
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
        .merge(routes::route_loop::router(planner))
        .merge(routes::photos::router(None))
        .merge(routes::fitness::router())
        .with_state(state)
        .layer(axum::middleware::from_fn(move |request, next| {
            auth::auth_middleware(request, next, key.clone())
        }));
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    println!("http://{}", listener.local_addr()?);
    axum::serve(listener, app).await?;
    Ok(())
}

/// Answer round trips on loopback in openrouteservice's response shape. Each seed draws a square
/// in its own direction from the requested origin, slightly off the requested length, so a
/// search finds distinct alternatives. Squares follow no real streets.
async fn synthetic_provider() -> anyhow::Result<ProviderEndpoint> {
    let provider = axum::Router::new().route(
        "/ors/v2/directions/foot-walking/geojson",
        post(|Json(request): Json<Value>| async move { Json(synthetic_trip(&request)) }),
    );
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let base = format!("http://{}/ors", listener.local_addr()?);
    tokio::spawn(async move { axum::serve(listener, provider).await });
    ProviderEndpoint::parse(Some(&base))?
        .ok_or_else(|| anyhow::anyhow!("synthetic provider URL is empty"))
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

fn synthetic_trip(request: &Value) -> Value {
    let origin = [
        request["coordinates"][0][0].as_f64().unwrap_or_default(),
        request["coordinates"][0][1].as_f64().unwrap_or_default(),
    ];
    let round_trip = &request["options"]["round_trip"];
    let length = round_trip["length"].as_f64().unwrap_or_default();
    // Direction east/north and length share per seed.
    let ([east, north], share) = match round_trip["seed"].as_u64() {
        Some(1) => ([1.0, 1.0], 1.02),
        Some(2) => ([-1.0, 1.0], 0.97),
        Some(3) => ([-1.0, -1.0], 1.06),
        _ => ([1.0, -1.0], 0.9),
    };
    let side = length * share / 4.0;
    let north = north * side / 111_195.0;
    let east = east * side / (111_195.0 * origin[1].to_radians().cos());
    let corners = [
        origin,
        [origin[0], origin[1] + north],
        [origin[0] + east, origin[1] + north],
        [origin[0] + east, origin[1]],
        origin,
    ];
    let coordinates = std::iter::once(origin)
        .chain(corners.windows(2).flat_map(|pair| {
            (1..=30).map(move |step| {
                let t = f64::from(step) / 30.0;
                [
                    pair[0][0] + (pair[1][0] - pair[0][0]) * t,
                    pair[0][1] + (pair[1][1] - pair[0][1]) * t,
                ]
            })
        }))
        .collect::<Vec<_>>();
    json!({
        "type": "FeatureCollection",
        "features": [{
            "type": "Feature",
            "geometry": { "type": "LineString", "coordinates": coordinates },
            "properties": { "summary": { "distance": length * share, "duration": length * share * 0.72 } }
        }]
    })
}
