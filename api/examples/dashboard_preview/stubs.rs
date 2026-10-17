//! Loopback stand-ins for the upstreams the API integrates with. Each speaks the real upstream's
//! wire shape, so the production Integration Services run unchanged.

use std::collections::HashMap;

use axum::{
    Form, Json, Router,
    extract::Path,
    routing::{get, post},
};
use chrono::{NaiveTime, Offset, Utc};
use chrono_tz::Europe::Copenhagen;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::net::TcpListener;
use url::Url;

pub async fn serve(router: Router) -> anyhow::Result<String> {
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
