//! A loopback Immich whose albums hold drawn scenes: no photographs, no people, no places.
//! Every photo has its own aspect ratio and capture time, so the gallery's justified rows,
//! album date spans and the merged newest-first view are all exercised. A few more photos fall
//! on fixed month-days in earlier years, including today's, so "on this day" always has matches.

use std::collections::HashMap;

use axum::{
    Json, Router,
    extract::{Path, Query},
    http::{StatusCode, header},
    response::IntoResponse,
    routing::get,
};
use chrono::{Datelike, Duration, NaiveDate};
use serde::Deserialize;
use serde_json::json;
use url::Url;
use uuid::Uuid;

mod scene;

use scene::picture;

#[derive(Deserialize)]
struct PhotoSeed {
    albums: Vec<AlbumSeed>,
    on_this_day: Vec<DaySeed>,
}

/// Photos taken on one month-day in earlier years, spread over the albums.
#[derive(Deserialize)]
struct DaySeed {
    /// `MM-DD`, or `today` for the household's date when the preview starts.
    day: String,
    years_ago: Vec<i32>,
    /// Photos for each of those years.
    photos: u32,
}

#[derive(Deserialize, Clone)]
struct AlbumSeed {
    name: String,
    photos: u32,
    hue: f64,
    /// sea, bloom, city, snow, forest or hills.
    scene: String,
    /// The month (`YYYY-MM`) of the album's first photo.
    from: String,
    months: u32,
    /// The owner's chosen cover, as a photo number; otherwise Immich's first photo.
    cover: Option<u32>,
}

fn photo_seed() -> PhotoSeed {
    serde_json::from_str(include_str!("seed/photos.json")).expect("photos.json is valid")
}

/// Version 4 and RFC 4122 variant bits keep the ids valid UUIDs; the low 48 bits carry the
/// album and photo numbers, and bit 48 tells albums from photos.
fn tagged(low: u128) -> Uuid {
    let version = 4u128 << 76;
    let variant = 0b10u128 << 62;
    Uuid::from_u128(0x5eed_0000_0000_0000_0000_0000_0000_0000 | version | variant | low)
}

fn album_id(album: usize) -> Uuid {
    tagged((1u128 << 48) | ((album as u128) << 32))
}

fn asset_id(album: usize, photo: u32) -> Uuid {
    tagged(((album as u128) << 32) | u128::from(photo))
}

/// The selected albums: every seeded album, or the first `OPENHOME_PREVIEW_ALBUMS` of them
/// (set it to 1 to preview a household with a single album).
pub fn album_ids() -> Vec<Uuid> {
    let seeded = photo_seed().albums.len();
    let count = std::env::var("OPENHOME_PREVIEW_ALBUMS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .map_or(seeded, |count| count.clamp(1, seeded));
    (0..count).map(album_id).collect()
}

/// A deterministic value in [0, 1) for any seed (splitmix64).
fn rand(seed: u64) -> f64 {
    let mut z = seed.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    (z ^ (z >> 31)) as f64 / u64::MAX as f64
}

fn photo_key(album: usize, photo: u32) -> u64 {
    (album as u64) << 32 | u64::from(photo)
}

/// Mostly landscape, with a real share of portrait, square and wide frames.
fn ratio(album: usize, photo: u32) -> (u32, u32) {
    const FRAMES: [(u32, u32); 10] = [
        (3, 2),
        (4, 3),
        (3, 2),
        (2, 3),
        (16, 9),
        (3, 4),
        (1, 1),
        (4, 3),
        (4, 5),
        (2, 3),
    ];
    FRAMES[(rand(photo_key(album, photo) ^ 0xf0) * FRAMES.len() as f64) as usize]
}

/// The newest photo comes first, as an Immich album sorted newest first lists it.
fn taken(album: &AlbumSeed, key: u64, photo: u32) -> String {
    let start =
        NaiveDate::parse_from_str(&format!("{}-01", album.from), "%Y-%m-%d").expect("YYYY-MM");
    let span = i64::from(album.months.max(1)) * 30 - 1;
    let step = span * i64::from(album.photos - 1 - photo) / i64::from(album.photos.max(2) - 1);
    let minutes = 7 * 60 + (rand(key ^ 0x7e) * 13.0 * 60.0) as i64;
    let time = start.and_hms_opt(0, 0, 0).expect("midnight")
        + Duration::days(step)
        + Duration::minutes(minutes);
    format!("{}.000Z", time.format("%Y-%m-%dT%H:%M:%S"))
}

/// Each album's anniversary photos, numbered after its regular ones, as (number, capture time).
/// Noon UTC keeps every one on its day in Copenhagen.
fn dated_photos(seed: &PhotoSeed, album: usize) -> Vec<(u32, String)> {
    let today = openhome_api::services::photos::copenhagen_today(chrono::Utc::now());
    let mut next = seed.albums[album].photos;
    let mut slot = 0usize;
    let mut photos = Vec::new();
    for entry in &seed.on_this_day {
        let day = if entry.day == "today" {
            today
        } else {
            NaiveDate::parse_from_str(&format!("2000-{}", entry.day), "%Y-%m-%d")
                .expect("on_this_day days are MM-DD")
        };
        for &ago in &entry.years_ago {
            let year = today.year() - ago;
            // 29 February in a common year is kept on the 28th, where "on this day" finds it.
            let Some(date) = day
                .with_year(year)
                .or_else(|| NaiveDate::from_ymd_opt(year, 2, 28))
            else {
                continue;
            };
            for _ in 0..entry.photos {
                if slot % seed.albums.len() == album {
                    photos.push((next, format!("{}T12:00:00.000Z", date.format("%Y-%m-%d"))));
                    next += 1;
                }
                slot += 1;
            }
        }
    }
    photos
}

pub async fn immich() -> anyhow::Result<Url> {
    let router = Router::new()
        .route(
            "/api/albums/{id}",
            get(|Path(id): Path<Uuid>| async move {
                let seed = photo_seed();
                let Some((index, album)) = seed
                    .albums
                    .iter()
                    .enumerate()
                    .find(|(index, _)| album_id(*index) == id)
                else {
                    return StatusCode::NOT_FOUND.into_response();
                };
                Json(json!({
                    "id": id,
                    "albumName": album.name,
                    "albumThumbnailAssetId": album.cover.map(|photo| asset_id(index, photo)),
                    "assets": (0..album.photos)
                        .map(|photo| (photo, taken(album, photo_key(index, photo), photo)))
                        .chain(dated_photos(&seed, index))
                        .map(|(photo, taken)| json!({
                            "id": asset_id(index, photo),
                            "type": "IMAGE",
                            "localDateTime": taken,
                        }))
                        .collect::<Vec<_>>(),
                }))
                .into_response()
            }),
        )
        .route(
            "/api/assets/{id}/thumbnail",
            get(
                |Path(id): Path<Uuid>, Query(query): Query<HashMap<String, String>>| async move {
                    let seed = photo_seed();
                    let raw = id.as_u128();
                    let index = ((raw >> 32) & 0xffff) as usize;
                    let photo = (raw & 0xffff_ffff) as u32;
                    let Some(album) = seed.albums.get(index) else {
                        return StatusCode::NOT_FOUND.into_response();
                    };
                    // Immich keeps the frame: a small short side for thumbnails, a larger one for previews.
                    let short = if query.get("size").map(String::as_str) == Some("preview") {
                        810
                    } else {
                        300
                    };
                    let (w, h) = ratio(index, photo);
                    let (width, height) = if w >= h {
                        (short * w / h, short)
                    } else {
                        (short, short * h / w)
                    };
                    (
                        [(header::CONTENT_TYPE, "image/png")],
                        picture(width, height, album, photo_key(index, photo)),
                    )
                        .into_response()
                },
            ),
        );
    Ok(Url::parse(&crate::stubs::serve(router).await?)?)
}
