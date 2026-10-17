//! A loopback Immich whose albums hold drawn scenes: no photographs, no people, no places.
//! Every photo has its own aspect ratio and capture time, so the gallery's justified rows,
//! album date spans and the merged newest-first view are all exercised. A few more photos fall
//! on fixed month-days in earlier years, including today's, so "on this day" always has matches.

use std::{collections::HashMap, f64::consts::TAU, io::Write};

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

type Rgb = [f64; 3];

fn hsl(hue: f64, saturation: f64, lightness: f64) -> Rgb {
    let hue = hue.rem_euclid(360.0);
    let lightness = lightness.clamp(0.0, 1.0);
    let chroma = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation.clamp(0.0, 1.0);
    let x = chroma * (1.0 - ((hue / 60.0) % 2.0 - 1.0).abs());
    let m = lightness - chroma / 2.0;
    let (r, g, b) = match (hue / 60.0) as u32 {
        0 => (chroma, x, 0.0),
        1 => (x, chroma, 0.0),
        2 => (0.0, chroma, x),
        3 => (0.0, x, chroma),
        4 => (x, 0.0, chroma),
        _ => (chroma, 0.0, x),
    };
    [r + m, g + m, b + m]
}

fn mix(a: Rgb, b: Rgb, t: f64) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    std::array::from_fn(|i| a[i] * (1.0 - t) + b[i] * t)
}

/// One photo's fixed choices: its light, its sun and its layout.
struct Frame {
    hue: f64,
    key: u64,
    aspect: f64,
    /// 0 is midday, 1 is a low evening sun.
    dusk: f64,
    sun: (f64, f64),
}

impl Frame {
    fn pick(&self, salt: u64) -> f64 {
        rand(self.key.wrapping_mul(31).wrapping_add(salt))
    }

    /// A sky that warms and deepens towards the horizon, with soft bands of cloud.
    fn sky(&self, x: f64, v: f64, horizon: f64) -> Rgb {
        let height = (v / horizon).clamp(0.0, 1.0);
        let top = hsl(
            self.hue + 8.0 - 20.0 * self.dusk,
            0.5,
            0.42 + 0.1 * (1.0 - self.dusk),
        );
        let low = hsl(
            self.hue - 40.0 * self.dusk + 10.0,
            0.55 + 0.25 * self.dusk,
            0.82 - 0.1 * self.dusk,
        );
        let mut color = mix(top, low, height.powf(1.6));
        let cloud = ((x * 3.1 + self.pick(1) * 9.0).sin() * (v * 17.0 + x * 1.3).sin()
            + (x * 7.3 + self.pick(2) * 5.0).sin() * 0.4)
            .max(0.0);
        if self.pick(3) > 0.35 {
            color = mix(
                color,
                hsl(self.hue + 30.0, 0.25, 0.94),
                cloud * 0.28 * (1.0 - height),
            );
        }
        let (sx, sy) = self.sun;
        let distance = ((x - sx).powi(2) + (v - sy).powi(2)).sqrt();
        let sun = hsl(28.0 + 22.0 * (1.0 - self.dusk), 0.95, 0.9);
        if distance < 0.055 {
            sun
        } else {
            mix(color, sun, (0.32 - distance).max(0.0) * 1.4)
        }
    }

    /// Haze: far things fade towards the sky's own colour at the horizon.
    fn haze(&self, color: Rgb, depth: f64) -> Rgb {
        mix(color, hsl(self.hue - 20.0 * self.dusk, 0.35, 0.8), depth)
    }
}

fn ridge(x: f64, base: f64, amplitude: f64, frequency: f64, phase: f64) -> f64 {
    base + amplitude * (x * frequency + phase).sin()
        + amplitude * 0.45 * (x * frequency * 2.3 + phase * 1.7).sin()
        + amplitude * 0.2 * (x * frequency * 5.1 + phase * 0.3).sin()
}

fn hills(frame: &Frame, x: f64, v: f64, snow: bool) -> Rgb {
    let mut color = frame.sky(x, v, 0.62);
    for layer in 0..4u64 {
        let depth = layer as f64;
        let top = ridge(
            x,
            0.42 + 0.11 * depth,
            0.09 - 0.012 * depth,
            2.0 + depth * 1.4,
            frame.pick(10 + layer) * TAU,
        );
        if v > top {
            let lightness = if snow {
                0.9 - 0.1 * depth
            } else {
                0.48 - 0.1 * depth
            };
            let saturation = if snow { 0.12 } else { 0.38 };
            let mut ground = hsl(frame.hue + 15.0 + depth * 8.0, saturation, lightness);
            // Light from the sun's side, shade on the other.
            let slope = (x * (2.0 + depth * 1.4) + frame.pick(10 + layer) * TAU).cos();
            ground = mix(
                ground,
                hsl(frame.hue + 200.0, 0.2, 0.2),
                (slope * 0.18 + 0.1).max(0.0),
            );
            if snow && v - top < 0.02 + 0.03 * frame.pick(20 + layer) {
                ground = hsl(frame.hue, 0.1, 0.97);
            }
            color = frame.haze(ground, (3.0 - depth) * 0.16);
        }
    }
    color
}

fn sea(frame: &Frame, x: f64, v: f64) -> Rgb {
    let horizon = 0.5 + 0.12 * frame.pick(30);
    let island = horizon
        - 0.07 * frame.pick(31) * (-((x - frame.pick(32) * frame.aspect) / 0.22).powi(2)).exp();
    if v < island {
        return frame.sky(x, v, horizon);
    }
    if v < horizon {
        // A low island on the horizon.
        return frame.haze(hsl(frame.hue + 140.0, 0.25, 0.32), 0.45);
    }
    let depth = (v - horizon) / (1.0 - horizon);
    let mut water = mix(
        hsl(frame.hue - 10.0 * frame.dusk, 0.45, 0.7),
        hsl(frame.hue + 10.0, 0.55, 0.26),
        depth.powf(0.7),
    );
    let swell = (v * 140.0 / (0.3 + depth) + (x * 9.0).sin() * 2.0).sin();
    water = mix(
        water,
        hsl(frame.hue, 0.3, 0.85),
        (swell - 0.6).max(0.0) * 0.5,
    );
    let (sx, _) = frame.sun;
    let column = 0.03 + 0.18 * depth;
    if (x - sx).abs() < column && swell > -0.2 {
        water = mix(
            water,
            hsl(36.0, 0.9, 0.88),
            0.55 * (1.0 - (x - sx).abs() / column),
        );
    }
    // Beach in the near corner on some frames.
    if frame.pick(33) > 0.5 && v > 0.9 - 0.12 * (x / frame.aspect) {
        water = hsl(38.0, 0.35, 0.72 - 0.12 * depth);
    }
    water
}

fn city(frame: &Frame, x: f64, v: f64) -> Rgb {
    let mut color = frame.sky(x, v, 0.75);
    for layer in 0..2u64 {
        let depth = layer as f64;
        let width = 0.11 - 0.03 * depth;
        let column = (x / width).floor();
        let cell = column as i64 as u64;
        let top = 0.32 + 0.1 * depth + 0.32 * frame.pick(100 + layer * 997 + cell);
        if v > top {
            let lightness = 0.2 + 0.08 * frame.pick(200 + cell) - 0.06 * depth;
            let mut wall = hsl(frame.hue + 200.0, 0.18, lightness);
            let fx = (x / width).fract();
            let row = ((v - top) / 0.035).floor() as u64;
            let slot = (fx * 5.0).floor() as u64;
            let inside = (fx * 5.0).fract() > 0.3 && ((v - top) / 0.035).fract() > 0.35;
            if fx > 0.08 && fx < 0.92 && inside {
                let lit = frame.pick(300 + cell * 131 + row * 17 + slot) > 0.55 - 0.3 * frame.dusk;
                wall = if lit {
                    hsl(38.0 + 10.0 * frame.pick(400 + row), 0.85, 0.66)
                } else {
                    mix(wall, hsl(frame.hue, 0.3, 0.5), 0.25)
                };
            }
            color = frame.haze(wall, if layer == 0 { 0.35 } else { 0.0 });
        }
    }
    // A street of warm light at the bottom edge.
    if v > 0.93 {
        color = mix(
            hsl(frame.hue + 200.0, 0.15, 0.14),
            hsl(36.0, 0.8, 0.6),
            (x * 13.0).sin().max(0.0) * 0.4,
        );
    }
    color
}

fn forest(frame: &Frame, x: f64, v: f64) -> Rgb {
    let mut color = hills(frame, x, v.min(0.6), false);
    if v > 0.6 {
        color = frame.haze(hsl(frame.hue + 10.0, 0.35, 0.36), 0.3);
    }
    for layer in 0..3u64 {
        let depth = layer as f64;
        let spacing = 0.1 + 0.05 * depth;
        let index = (x / spacing).floor();
        let cell = index as i64 as u64;
        let centre = (index + 0.3 + 0.4 * frame.pick(500 + layer * 991 + cell)) * spacing;
        let base = 0.66 + 0.13 * depth;
        let height = 0.22 + 0.14 * depth + 0.12 * frame.pick(600 + cell);
        let top = base - height;
        if v > top && v < base + 0.02 {
            let half = (v - top) / height * spacing * 0.42;
            if (x - centre).abs() < half {
                let shade = if x < centre { 0.06 } else { 0.0 };
                let tree = hsl(frame.hue + 25.0, 0.4, 0.3 - 0.07 * depth + shade);
                color = frame.haze(tree, (2.0 - depth) * 0.22);
            }
        }
        if layer == 2 && v > base {
            color = hsl(frame.hue + 5.0, 0.3, 0.2);
        }
    }
    color
}

fn bloom(frame: &Frame, x: f64, v: f64) -> Rgb {
    let leaves = f64::midpoint(
        (x * 23.0).sin() * (v * 19.0).cos(),
        (x * 7.0 + v * 11.0).sin(),
    );
    let mut color = mix(
        hsl(frame.hue + 20.0, 0.35, 0.62),
        hsl(frame.hue, 0.45, 0.2),
        v.powf(0.8) + leaves * 0.12,
    );
    let size = 0.16;
    // Flowers reach into neighbouring cells, so each pixel checks the 3x3 cells around it.
    for (dx, dy) in (0..9).map(|i| (f64::from(i % 3) - 1.0, f64::from(i / 3) - 1.0)) {
        let cx = (x / size).floor() + dx;
        let cy = (v / size).floor() + dy;
        // Shifted by one so the cells left of and above the frame stay non-negative.
        let cell = (cx + 1.0) as u64 * 7919 + (cy + 1.0) as u64;
        if frame.pick(700 + cell) < 0.35 {
            continue;
        }
        let px = (cx + 0.5 + 0.8 * (frame.pick(800 + cell) - 0.5)) * size;
        let py = (cy + 0.5 + 0.8 * (frame.pick(900 + cell) - 0.5)) * size;
        let scale = 0.3 + 0.7 * py.clamp(0.0, 1.0);
        let radius = size * (0.28 + 0.2 * frame.pick(1000 + cell)) * scale * 1.6;
        let (ox, oy) = (x - px, v - py);
        let distance = (ox * ox + oy * oy).sqrt();
        let angle = oy.atan2(ox) + frame.pick(1100 + cell) * TAU;
        let petals = 5.0 + (frame.pick(1200 + cell) * 3.0).floor();
        let edge = radius * (0.72 + 0.28 * (angle * petals).cos().abs());
        if distance < edge {
            let palette = [frame.hue + 200.0, 340.0, 48.0, 0.0, 300.0];
            let hue = palette[(frame.pick(1300 + cell) * palette.len() as f64) as usize];
            let white = frame.pick(1300 + cell) > 0.8;
            color = if distance < radius * 0.22 {
                hsl(44.0, 0.9, 0.55)
            } else if white {
                hsl(hue, 0.1, 0.94 - 0.15 * distance / edge)
            } else {
                hsl(hue, 0.7, 0.72 - 0.22 * distance / edge)
            };
        }
    }
    color
}

fn picture(width: u32, height: u32, album: &AlbumSeed, key: u64) -> Vec<u8> {
    let aspect = f64::from(width) / f64::from(height);
    let seed = key ^ (album.hue as u64) << 40;
    let dusk = rand(seed ^ 0xd5);
    let frame = Frame {
        hue: album.hue + 30.0 * (rand(seed ^ 0x11) - 0.5),
        key: seed,
        aspect,
        dusk,
        sun: (
            aspect * (0.15 + 0.7 * rand(seed ^ 0x22)),
            0.12 + 0.25 * dusk + 0.05 * rand(seed ^ 0x33),
        ),
    };
    let mut pixels = Vec::with_capacity((width * height * 3) as usize);
    for y in 0..height {
        let v = f64::from(y) / f64::from(height);
        for column in 0..width {
            // In height units, so circles stay round in every frame.
            let x = f64::from(column) / f64::from(height);
            let color = match album.scene.as_str() {
                "sea" => sea(&frame, x, v),
                "city" => city(&frame, x, v),
                "forest" => forest(&frame, x, v),
                "bloom" => bloom(&frame, x, v),
                "snow" => hills(&frame, x, v, true),
                _ => hills(&frame, x, v, false),
            };
            pixels.extend(color.map(|channel| (channel.clamp(0.0, 1.0) * 255.0) as u8));
        }
    }
    png(width, height, &pixels)
}

fn png(width: u32, height: u32, rgb: &[u8]) -> Vec<u8> {
    fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
        out.extend((data.len() as u32).to_be_bytes());
        out.extend(kind);
        out.extend(data);
        let mut crc = flate2::Crc::new();
        crc.update(kind);
        crc.update(data);
        out.extend(crc.sum().to_be_bytes());
    }
    let mut raw = Vec::with_capacity(rgb.len() + height as usize);
    for row in rgb.chunks((width * 3) as usize) {
        raw.push(0);
        raw.extend(row);
    }
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
    encoder.write_all(&raw).expect("writing to memory");
    let compressed = encoder.finish().expect("compressing in memory");
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut header = Vec::new();
    header.extend(width.to_be_bytes());
    header.extend(height.to_be_bytes());
    header.extend([8, 2, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &header);
    chunk(&mut out, b"IDAT", &compressed);
    chunk(&mut out, b"IEND", &[]);
    out
}
