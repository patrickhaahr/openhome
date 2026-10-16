//! Synthetic loopback upstreams shared by the dashboard examples: openrouteservice-shaped walking
//! loops and TMDB-shaped titles. Nothing here is a real path, title or household record.

use axum::{
    Json,
    extract::{Path, Query},
    http::StatusCode,
    routing::{get, post},
};
use openhome_api::services::{route_loop::ProviderEndpoint, tmdb::TmdbService};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::net::TcpListener;

/// A TMDB service backed by a loopback provider that knows two fictional titles.
pub async fn synthetic_tmdb() -> anyhow::Result<TmdbService> {
    let provider = axum::Router::new()
        .route("/3/search/{kind}", get(provider_search))
        .route("/3/{kind}/{id}", get(provider_details));
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let url = format!("http://{}/3/", listener.local_addr()?);
    tokio::spawn(async move { axum::serve(listener, provider).await });
    TmdbService::new("synthetic-provider-token", url.parse()?)
}

/// Answer round trips on loopback in openrouteservice's response shape. Each seed draws a square
/// in its own direction from the requested origin, slightly off the requested length, so a
/// search finds distinct alternatives. Squares follow no real streets.
pub async fn synthetic_provider() -> anyhow::Result<ProviderEndpoint> {
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
