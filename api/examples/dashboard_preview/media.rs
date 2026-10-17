//! The preview's pictures and its TMDB fallback. Wish images are drawn here and answered through
//! the authenticated wish picture route, so they work offline. Posters are
//! real TMDB images: the seeded Watchlist names well-known titles with their static TMDB ids and
//! poster paths from `seed/household.json`, never fetched at startup.

use axum::{
    Json, Router,
    extract::{Path, Query},
    http::{StatusCode, header},
    response::IntoResponse,
    routing::get,
};
use openhome_api::services::tmdb::TmdbService;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::net::TcpListener;

/// One catalogue title, as TMDB would describe it.
#[derive(Deserialize, Clone)]
pub struct Title {
    pub id: i64,
    pub kind: String,
    pub title: String,
    pub date: String,
    pub poster_path: String,
    pub overview: String,
}

pub fn catalogue() -> Vec<Title> {
    #[derive(Deserialize)]
    struct Seed {
        catalogue: Vec<Title>,
    }
    serde_json::from_str::<Seed>(include_str!("seed/household.json"))
        .expect("seed/household.json has a catalogue")
        .catalogue
}

fn provider_kind(kind: &str) -> &str {
    if kind == "series" { "tv" } else { "movie" }
}

fn provider_title(title: &Title) -> Value {
    let mut item = json!({
        "id": title.id,
        "overview": title.overview,
        "poster_path": title.poster_path,
    });
    if title.kind == "movie" {
        item["title"] = json!(title.title);
        item["release_date"] = json!(title.date);
    } else {
        item["name"] = json!(title.title);
        item["first_air_date"] = json!(title.date);
    }
    item
}

#[derive(Deserialize)]
struct Search {
    query: String,
}

/// The real TMDB when `TMDB_READ_ACCESS_TOKEN` is set (built as `api/src/main.rs` does), else a
/// loopback stand-in that searches the seed catalogue. Only the chosen mode is logged.
pub async fn tmdb() -> anyhow::Result<TmdbService> {
    if let Some(token) = std::env::var("TMDB_READ_ACCESS_TOKEN")
        .ok()
        .filter(|token| !token.trim().is_empty())
    {
        let base_url = url::Url::parse("https://api.themoviedb.org/3/")?;
        match TmdbService::new(&token, base_url) {
            Ok(service) => {
                eprintln!("TMDB: live search (token from the environment)");
                return Ok(service);
            }
            Err(_) => eprintln!("TMDB configuration is invalid; using the loopback catalogue"),
        }
    } else {
        eprintln!("TMDB: loopback catalogue (no TMDB_READ_ACCESS_TOKEN)");
    }
    loopback_tmdb().await
}

/// TMDB search and details over the seed catalogue. A query containing "offline" answers 503,
/// so the add flow's provider failure can be previewed too.
async fn loopback_tmdb() -> anyhow::Result<TmdbService> {
    let provider = Router::new()
        .route(
            "/3/search/{kind}",
            get(
                |Path(kind): Path<String>, Query(search): Query<Search>| async move {
                    let query = search.query.to_lowercase();
                    if query.contains("offline") {
                        return Err(StatusCode::SERVICE_UNAVAILABLE);
                    }
                    let results: Vec<Value> = catalogue()
                        .iter()
                        .filter(|title| provider_kind(&title.kind) == kind)
                        .filter(|title| title.title.to_lowercase().contains(query.trim()))
                        .map(provider_title)
                        .collect();
                    Ok(Json(json!({ "results": results })))
                },
            ),
        )
        .route(
            "/3/{kind}/{id}",
            get(|Path((kind, id)): Path<(String, i64)>| async move {
                catalogue()
                    .iter()
                    .find(|title| title.id == id && provider_kind(&title.kind) == kind)
                    .map(|title| Json(provider_title(title)))
                    .ok_or(StatusCode::NOT_FOUND)
            }),
        );
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let url = format!("http://{}/3/", listener.local_addr()?);
    tokio::spawn(async move { axum::serve(listener, provider).await });
    TmdbService::new("synthetic-provider-token", url.parse()?)
}

/// The preview's stand-in for the wish picture route. The production fetcher refuses loopback
/// addresses, so a seeded `/preview-art` picture is answered here from the drawing itself; every
/// other request continues to the real route. Mount it inside the API key layer.
pub async fn wish_picture(
    db: sqlx::SqlitePool,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let item = request
        .uri()
        .path()
        .strip_prefix("/api/household-users/")
        .and_then(|rest| rest.strip_suffix("/image"))
        .and_then(|rest| rest.rsplit_once("/items/"))
        .map(|(_, item)| item.to_owned());
    let Some(item) = item.filter(|_| request.method() == axum::http::Method::GET) else {
        return next.run(request).await;
    };
    let stored: Option<Option<String>> =
        sqlx::query_scalar("SELECT image_url FROM wishlist_items WHERE id = ?")
            .bind(&item)
            .fetch_optional(&db)
            .await
            .ok()
            .flatten();
    let drawing = stored
        .flatten()
        .and_then(|url| {
            url.split_once("/preview-art/wish/")
                .map(|(_, file)| file.to_owned())
        })
        .and_then(|file| file.strip_suffix(".svg").and_then(wish));
    match drawing {
        Some(body) => (
            [
                (header::CONTENT_TYPE, "image/svg+xml"),
                (header::CACHE_CONTROL, "private, max-age=3600"),
                (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
                (
                    header::CONTENT_SECURITY_POLICY,
                    "default-src 'none'; style-src 'unsafe-inline'; sandbox",
                ),
            ],
            body,
        )
            .into_response(),
        None => next.run(request).await,
    }
}

fn hsl(hue: f64, saturation: f64, lightness: f64) -> String {
    format!(
        "hsl({:.0} {saturation:.0}% {lightness:.0}%)",
        hue.rem_euclid(360.0)
    )
}

/// A still life of one product on a coloured ground. Every drawing is invented.
fn wish(name: &str) -> Option<String> {
    let (hue, object) = match name {
        "socks" => (
            18.0,
            r#"<path d="M190 120h90v190c0 30 20 50 60 60l40 12c40 12 50 70 10 86-30 12-70 6-110-10l-60-24c-30-12-40-40-40-70Z" fill="hsl(18 55% 46%)"/><rect x="190" y="120" width="90" height="34" fill="hsl(18 30% 92%)"/><path d="M320 150h90v170c0 30 18 46 52 56l30 10c36 12 42 64 6 78-28 10-62 4-96-10l-54-22c-26-10-34-36-34-62Z" fill="hsl(28 60% 56%)"/><rect x="320" y="150" width="90" height="34" fill="hsl(18 30% 92%)"/>"#,
        ),
        "tripod" => (
            210.0,
            r#"<rect x="240" y="120" width="120" height="74" rx="14" fill="hsl(210 20% 22%)"/><circle cx="300" cy="157" r="24" fill="hsl(210 30% 60%)"/><rect x="286" y="194" width="28" height="60" fill="hsl(210 15% 30%)"/><path d="M300 250 190 500M300 250 410 500M300 250v250" stroke="hsl(210 15% 26%)" stroke-width="16" stroke-linecap="round"/>"#,
        ),
        "headphones" => (
            340.0,
            r#"<path d="M170 340v-50a130 130 0 0 1 260 0v50" fill="none" stroke="hsl(340 20% 20%)" stroke-width="30" stroke-linecap="round"/><rect x="140" y="320" width="80" height="140" rx="36" fill="hsl(340 45% 42%)"/><rect x="380" y="320" width="80" height="140" rx="36" fill="hsl(340 45% 42%)"/>"#,
        ),
        "skillet" => (
            30.0,
            r#"<circle cx="260" cy="320" r="150" fill="hsl(30 10% 16%)"/><circle cx="260" cy="320" r="118" fill="hsl(30 8% 24%)"/><rect x="400" y="300" width="170" height="40" rx="20" fill="hsl(30 10% 16%)"/>"#,
        ),
        "pourover" => (
            160.0,
            r#"<path d="M200 150h200l-60 130h-80Z" fill="hsl(160 8% 52%)"/><rect x="186" y="134" width="228" height="26" rx="10" fill="hsl(160 8% 44%)"/><path d="M230 290h140c20 0 30 40 30 100v60c0 30-20 50-50 50H250c-30 0-50-20-50-50v-60c0-60 10-100 30-100Z" fill="hsl(160 20% 88%)" opacity=".9"/><path d="M212 400h176v50c0 30-20 50-50 50h-76c-30 0-50-20-50-50Z" fill="hsl(28 50% 34%)"/>"#,
        ),
        "towels" => (
            90.0,
            r#"<rect x="140" y="350" width="320" height="80" rx="18" fill="hsl(90 18% 84%)"/><rect x="150" y="270" width="300" height="80" rx="18" fill="hsl(40 30% 90%)"/><rect x="160" y="190" width="280" height="80" rx="18" fill="hsl(90 18% 80%)"/><rect x="160" y="222" width="280" height="12" fill="hsl(90 30% 40%)"/><rect x="150" y="302" width="300" height="12" fill="hsl(14 50% 50%)"/><rect x="140" y="382" width="320" height="12" fill="hsl(90 30% 40%)"/>"#,
        ),
        "lamp" => (
            44.0,
            r#"<path d="M220 130h160l50 140H170Z" fill="hsl(44 70% 62%)"/><rect x="292" y="270" width="16" height="210" fill="hsl(44 10% 22%)"/><ellipse cx="300" cy="490" rx="90" ry="20" fill="hsl(44 10% 22%)"/><path d="M200 270h200l40 60H160Z" fill="hsl(48 90% 90%)" opacity=".5"/>"#,
        ),
        "atlas" => (
            200.0,
            r#"<rect x="170" y="110" width="260" height="380" rx="10" fill="hsl(200 50% 30%)"/><rect x="170" y="110" width="30" height="380" fill="hsl(200 50% 22%)"/><circle cx="315" cy="270" r="80" fill="none" stroke="hsl(40 70% 80%)" stroke-width="8"/><path d="M235 270h160M315 190v160" stroke="hsl(40 70% 80%)" stroke-width="6"/><rect x="240" y="400" width="150" height="14" fill="hsl(40 70% 80%)"/>"#,
        ),
        "almanac" => (
            12.0,
            r#"<rect x="190" y="120" width="230" height="360" rx="8" fill="hsl(12 55% 44%)"/><rect x="190" y="120" width="26" height="360" fill="hsl(12 55% 34%)"/><path d="M250 330c40-60 100-60 140 0" fill="none" stroke="hsl(40 60% 88%)" stroke-width="8"/><path d="M240 360h160" stroke="hsl(40 60% 88%)" stroke-width="8"/><rect x="250" y="190" width="120" height="12" fill="hsl(40 60% 88%)"/><rect x="250" y="214" width="80" height="12" fill="hsl(40 60% 88%)"/>"#,
        ),
        "saw" => (
            100.0,
            r#"<path d="M120 300 420 250v70L120 340Z" fill="hsl(100 4% 70%)"/><path d="M120 340l15-12 15 10 15-12 15 10 15-12 15 10 15-12 15 10 15-12 15 10 15-12 15 10 15-12 15 10 15-12 15 10 15-12 15 10 15-12 15 10 15-12" fill="none" stroke="hsl(100 4% 50%)" stroke-width="4"/><rect x="420" y="262" width="120" height="44" rx="20" fill="hsl(28 40% 40%)"/><path d="M440 270v28M462 270v28M484 270v28" stroke="hsl(28 30% 26%)" stroke-width="4"/>"#,
        ),
        "apron" => (
            80.0,
            r#"<path d="M230 160h140v40c0 30 30 40 70 50v210c0 20-10 30-30 30H190c-20 0-30-10-30-30V250c40-10 70-20 70-50Z" fill="hsl(80 30% 34%)"/><path d="M230 160c0-60 140-60 140 0" fill="none" stroke="hsl(30 30% 30%)" stroke-width="10"/><rect x="220" y="320" width="160" height="90" rx="10" fill="hsl(80 28% 28%)"/>"#,
        ),
        "plane" => (
            36.0,
            r#"<path d="M150 400 200 290h220l40 110Z" fill="hsl(210 10% 30%)"/><rect x="250" y="250" width="110" height="50" rx="12" fill="hsl(36 50% 46%)"/><circle cx="200" cy="300" r="34" fill="hsl(36 50% 40%)"/><rect x="140" y="400" width="330" height="30" rx="6" fill="hsl(210 10% 22%)"/>"#,
        ),
        "scarf" => (
            226.0,
            r#"<path d="M180 140c80 40 160 40 240 0v80c-80 40-160 40-240 0Z" fill="hsl(226 50% 34%)"/><path d="M220 210h70v280h-70Z" fill="hsl(226 50% 30%)"/><path d="M310 210h70v240h-70Z" fill="hsl(226 50% 40%)"/><path d="M220 470v40M240 470v40M260 470v40M280 470v40M310 430v40M330 430v40M350 430v40M370 430v40" stroke="hsl(226 50% 30%)" stroke-width="6"/>"#,
        ),
        "mug" => (
            170.0,
            r#"<rect x="190" y="220" width="190" height="200" rx="16" fill="hsl(170 30% 92%)"/><rect x="190" y="220" width="190" height="20" fill="hsl(170 30% 30%)"/><path d="M380 270h30a50 50 0 0 1 0 100h-30" fill="none" stroke="hsl(170 30% 92%)" stroke-width="26"/><circle cx="285" cy="320" r="32" fill="hsl(170 40% 46%)"/>"#,
        ),
        "jacket" => (
            50.0,
            r#"<path d="M240 140c20-30 100-30 120 0l90 60 40 220h-60l-20-120v230H190V300l-20 120h-60l40-220Z" fill="hsl(50 80% 52%)"/><path d="M300 150v330" stroke="hsl(50 60% 36%)" stroke-width="8"/><path d="M240 140c20 40 100 40 120 0" fill="none" stroke="hsl(50 60% 36%)" stroke-width="8"/>"#,
        ),
        "plant" => (
            120.0,
            r#"<path d="M210 360h180l-24 140H234Z" fill="hsl(18 55% 52%)"/><rect x="200" y="340" width="200" height="30" rx="6" fill="hsl(18 55% 46%)"/><ellipse cx="250" cy="250" rx="34" ry="90" transform="rotate(-28 250 250)" fill="hsl(120 35% 34%)"/><ellipse cx="350" cy="250" rx="34" ry="90" transform="rotate(28 350 250)" fill="hsl(120 35% 40%)"/><ellipse cx="300" cy="220" rx="30" ry="110" fill="hsl(120 35% 30%)"/>"#,
        ),
        _ => return None,
    };
    Some(format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 600 600" width="600" height="600"><rect width="600" height="600" fill="{}"/><ellipse cx="300" cy="520" rx="210" ry="22" fill="{}"/>{object}</svg>"#,
        hsl(hue, 34.0, 82.0),
        hsl(hue, 22.0, 70.0),
    ))
}
