//! Loopback stand-ins for the upstreams the API integrates with. Each speaks the real upstream's
//! wire shape, so the production Integration Services run unchanged.

use std::{collections::HashMap, io::Write};

use axum::{
    Form, Json, Router,
    extract::Path,
    http::{StatusCode, header},
    response::IntoResponse,
    routing::{get, post},
};
use chrono::{NaiveTime, Offset, Utc};
use chrono_tz::Europe::Copenhagen;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::net::TcpListener;
use url::Url;
use uuid::Uuid;

async fn serve(router: Router) -> anyhow::Result<String> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let base = format!("http://{}", listener.local_addr()?);
    tokio::spawn(async move { axum::serve(listener, router).await });
    Ok(base)
}

#[derive(Deserialize)]
struct Controls {
    lgtv: Vec<String>,
    edifier: Vec<String>,
}

/// The IR bridge reports the full command sets and accepts every command.
pub async fn ir_bridge() -> anyhow::Result<String> {
    let controls: Controls = serde_json::from_str(include_str!("seed/controls.json"))?;
    serve(
        Router::new()
            .route(
                "/remotes",
                get(move || async move {
                    Json(json!({
                        "message": "IR remote ready",
                        "remotes": { "edifier": controls.edifier, "lgtv": controls.lgtv },
                    }))
                }),
            )
            .route(
                "/remotes/{remote}",
                post(
                    |Path(remote): Path<String>, Form(form): Form<HashMap<String, String>>| async move {
                        let command = form.get("command").cloned().unwrap_or_default();
                        Json(json!({ "message": format!("Sent {command} to {remote} (synthetic)") }))
                    },
                ),
            ),
    )
    .await
}

/// The `SwitchBot` bridge accepts light commands without touching any device.
pub async fn switchbot_bridge() -> anyhow::Result<String> {
    serve(Router::new().route(
        "/lights/{state}",
        post(|Path(state): Path<String>| async move {
            Json(json!({ "message": format!("Lights {state} (synthetic)") }))
        }),
    ))
    .await
}

#[derive(Deserialize)]
struct WeatherSeed {
    current: CurrentSeed,
    days: Vec<DaySeed>,
    rain_hours_local: Vec<u32>,
}

#[derive(Deserialize)]
struct CurrentSeed {
    weather_code: u8,
    feels_like_offset_c: f64,
    wind_ms: f64,
}

#[derive(Deserialize)]
struct DaySeed {
    weather_code: u8,
    high_c: f64,
    low_c: f64,
    precipitation_mm: f64,
    wind_ms: f64,
}

/// An Open-Meteo forecast from `seed/weather.json`, dated from the current Copenhagen day.
pub async fn open_meteo() -> anyhow::Result<Url> {
    let seed: &'static WeatherSeed = Box::leak(Box::new(serde_json::from_str(include_str!(
        "seed/weather.json"
    ))?));
    let base = serve(Router::new().route(
        "/v1/forecast",
        get(move || async move { Json(forecast(seed)) }),
    ))
    .await?;
    Ok(Url::parse(&format!("{base}/v1/forecast"))?)
}

fn forecast(seed: &WeatherSeed) -> Value {
    let now = Utc::now();
    let local = now.with_timezone(&Copenhagen);
    let offset = i64::from(local.offset().fix().local_minus_utc());
    let first_day = local.date_naive();
    // Local midnight of `first_day` read as if it were UTC, so local clock arithmetic stays simple.
    let first_local_midnight = first_day.and_time(NaiveTime::MIN).and_utc().timestamp();
    let midnight = |index: i64| first_local_midnight + index * 86_400 - offset;
    let day_of = |timestamp: i64| {
        (timestamp + offset - first_local_midnight)
            .div_euclid(86_400)
            .clamp(0, 6) as usize
    };
    let hour_start = now.timestamp().div_euclid(3600) * 3600;
    // Daily temperature swings between the low at 03:00 and the high at 15:00.
    let temperature = |timestamp: i64| {
        let hour = (timestamp + offset).rem_euclid(86_400) as f64 / 3600.0;
        let day = &seed.days[day_of(timestamp)];
        let phase = 0.5 - 0.5 * ((hour - 3.0) / 24.0 * std::f64::consts::TAU).cos();
        ((day.low_c + (day.high_c - day.low_c) * phase) * 10.0).round() / 10.0
    };
    let hours: Vec<i64> = (0..48).map(|index| hour_start + index * 3600).collect();
    let rain_hour = |timestamp: i64| {
        let hour = ((timestamp + offset).rem_euclid(86_400) / 3600) as u32;
        seed.rain_hours_local.contains(&hour)
    };
    let rain_share = seed.rain_hours_local.len() as f64;
    json!({
        "latitude": 55.6869,
        "longitude": 12.5692,
        "utc_offset_seconds": offset,
        "timezone": "Europe/Copenhagen",
        "current_units": { "temperature_2m": "°C", "apparent_temperature": "°C", "wind_speed_10m": "m/s" },
        "current": {
            "time": now.timestamp(),
            "temperature_2m": temperature(now.timestamp()),
            "apparent_temperature": ((temperature(now.timestamp()) + seed.current.feels_like_offset_c) * 10.0).round() / 10.0,
            "weather_code": seed.current.weather_code,
            "wind_speed_10m": seed.current.wind_ms,
        },
        "hourly_units": {
            "temperature_2m": "°C", "wind_speed_10m": "m/s",
            "precipitation": "mm", "precipitation_probability": "%",
        },
        "hourly": {
            "time": hours,
            "temperature_2m": hours.iter().map(|t| temperature(*t)).collect::<Vec<_>>(),
            "wind_speed_10m": hours.iter().map(|t| {
                let day = &seed.days[day_of(*t)];
                (day.wind_ms * (0.7 + 0.3 * ((*t as f64) / 5400.0).sin()) * 10.0).round() / 10.0
            }).collect::<Vec<_>>(),
            "precipitation": hours.iter().map(|t| {
                let day = &seed.days[day_of(*t)];
                if rain_hour(*t) { (day.precipitation_mm / rain_share * 10.0).round() / 10.0 } else { 0.0 }
            }).collect::<Vec<_>>(),
            "precipitation_probability": hours.iter().map(|t| {
                let day = &seed.days[day_of(*t)];
                if day.precipitation_mm <= 0.0 { 5.0 } else if rain_hour(*t) { 80.0 } else { 25.0 }
            }).collect::<Vec<_>>(),
        },
        "daily_units": {
            "temperature_2m_max": "°C", "temperature_2m_min": "°C",
            "precipitation_sum": "mm", "wind_speed_10m_max": "m/s",
        },
        "daily": {
            "time": (0..7).map(midnight).collect::<Vec<_>>(),
            "weather_code": seed.days.iter().map(|d| d.weather_code).collect::<Vec<_>>(),
            "temperature_2m_max": seed.days.iter().map(|d| d.high_c).collect::<Vec<_>>(),
            "temperature_2m_min": seed.days.iter().map(|d| d.low_c).collect::<Vec<_>>(),
            "precipitation_sum": seed.days.iter().map(|d| d.precipitation_mm).collect::<Vec<_>>(),
            "wind_speed_10m_max": seed.days.iter().map(|d| d.wind_ms).collect::<Vec<_>>(),
        },
    })
}

#[derive(Deserialize)]
struct PhotoSeed {
    albums: Vec<AlbumSeed>,
}

#[derive(Deserialize, Clone)]
struct AlbumSeed {
    name: String,
    photos: u32,
    hue: f64,
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

pub fn album_ids() -> Vec<Uuid> {
    (0..photo_seed().albums.len()).map(album_id).collect()
}

/// An Immich server whose albums hold generated landscapes: no photographs, no people.
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
                    "assets": (0..album.photos)
                        .map(|photo| json!({ "id": asset_id(index, photo), "type": "IMAGE" }))
                        .collect::<Vec<_>>(),
                }))
                .into_response()
            }),
        )
        .route(
            "/api/assets/{id}/thumbnail",
            get(
                |Path(id): Path<Uuid>, axum::extract::Query(query): axum::extract::Query<HashMap<String, String>>| async move {
                    let seed = photo_seed();
                    let raw = id.as_u128();
                    let album = ((raw >> 32) & 0xffff) as usize;
                    let photo = (raw & 0xffff_ffff) as u32;
                    let Some(album_seed) = seed.albums.get(album) else {
                        return StatusCode::NOT_FOUND.into_response();
                    };
                    let (width, height) = if query.get("size").map(String::as_str) == Some("preview") {
                        (1024, 768)
                    } else {
                        (360, 270)
                    };
                    (
                        [(header::CONTENT_TYPE, "image/png")],
                        landscape(width, height, album_seed.hue, photo),
                    )
                        .into_response()
                },
            ),
        );
    Ok(Url::parse(&serve(router).await?)?)
}

fn hsl(hue: f64, saturation: f64, lightness: f64) -> [f64; 3] {
    let hue = hue.rem_euclid(360.0);
    let chroma = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
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

/// A deterministic sky, sun and three layers of hills; every photo number draws a different scene.
fn landscape(width: u32, height: u32, hue: f64, photo: u32) -> Vec<u8> {
    let variation = f64::from(photo);
    let sun_x = 0.15 + 0.7 * ((variation * 0.618).fract());
    let sun_y = 0.18 + 0.2 * ((variation * 0.37).fract());
    let layers = [
        (0.58, 0.07, 3.0 + variation % 4.0, 0.62),
        (0.68, 0.09, 2.0 + variation % 3.0, 0.45),
        (0.80, 0.06, 4.0 + variation % 5.0, 0.30),
    ];
    let mut pixels = Vec::with_capacity((width * height * 3) as usize);
    for y in 0..height {
        let v = f64::from(y) / f64::from(height);
        for x in 0..width {
            let u = f64::from(x) / f64::from(width);
            let sky = hsl(hue + 25.0 * v, 0.55, 0.78 - 0.28 * (1.0 - v));
            let mut color = sky;
            let sun_distance = ((u - sun_x).powi(2) + ((v - sun_y) * 0.75).powi(2)).sqrt();
            if sun_distance < 0.07 {
                color = hsl(hue + 150.0, 0.8, 0.9);
            } else if sun_distance < 0.16 {
                let glow = (0.16 - sun_distance) / 0.09 * 0.35;
                let halo = hsl(hue + 150.0, 0.8, 0.9);
                color = std::array::from_fn(|i| color[i] * (1.0 - glow) + halo[i] * glow);
            }
            for (base, amplitude, frequency, lightness) in layers {
                let ridge = base
                    + amplitude * (u * frequency * std::f64::consts::TAU + variation).sin()
                    + amplitude * 0.4 * (u * frequency * 2.7 * std::f64::consts::TAU).sin();
                if v > ridge {
                    color = hsl(hue + 10.0, 0.4, lightness * (1.0 - 0.15 * (v - ridge)));
                }
            }
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
