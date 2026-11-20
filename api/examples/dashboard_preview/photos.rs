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

#[derive(Deserialize)]
struct DaySeed {
    day: String,
    years_ago: Vec<i32>,
    photos: u32,
}

#[derive(Deserialize, Clone)]
struct AlbumSeed {
    name: String,
    photos: u32,
    hue: f64,
    scene: String,
    from: String,
    months: u32,
    cover: Option<u32>,
}

fn photo_seed() -> PhotoSeed {
    serde_json::from_str(include_str!("seed/photos.json")).expect("photos.json is valid")
}

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

pub fn album_ids() -> Vec<Uuid> {
    let seeded = photo_seed().albums.len();
    let count = std::env::var("OPENHOME_PREVIEW_ALBUMS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .map_or(seeded, |count| count.clamp(1, seeded));
    (0..count).map(album_id).collect()
}

fn rand(seed: u64) -> f64 {
    let mut z = seed.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    (z ^ (z >> 31)) as f64 / u64::MAX as f64
}

fn photo_key(album: usize, photo: u32) -> u64 {
    (album as u64) << 32 | u64::from(photo)
}

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
