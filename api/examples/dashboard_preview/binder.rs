//! The preview's OPTCG catalogue stand-in: invented cards in the real catalogue's wire shape, with
//! PNG images served under `.jpg` names like the real thing. A variant's `card_image` names the
//! file the catalogue serves, which is not always its id (`OP99-020`'s special print). Every request is logged to stderr
//! (`OPTCG stand-in: GET <path> -> <status>`), so a verifier can count what Axum asked for in the
//! stack log. Code `OP99-503` simulates an outage; any other unknown code answers 404.

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
};
use chrono::{Duration, Utc};
use image::{ImageEncoder, Rgb, RgbImage, codecs::png::PngEncoder};
use openhome_api::services::optcg::Optcg;
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Deserialize)]
struct Seed {
    cards: Vec<SeedCard>,
}

#[derive(Deserialize, Clone)]
struct SeedCard {
    card_code: String,
    image_id: String,
    image_file: Option<String>,
    name: String,
    set_name: String,
    rarity: String,
    color: String,
    card_type: String,
    life: Option<String>,
    cost: Option<String>,
    power: Option<String>,
    counter: Option<String>,
    attribute: Option<String>,
    sub_types: String,
    inventory_price: f64,
    market_price: Option<f64>,
    observed_days_ago: i64,
    text: String,
}

fn catalogue() -> Vec<SeedCard> {
    serde_json::from_str::<Seed>(include_str!("seed/cards.json"))
        .expect("seed/cards.json is valid")
        .cards
}

impl SeedCard {
    fn file(&self) -> &str {
        self.image_file.as_deref().unwrap_or(&self.image_id)
    }
}

fn wire(card: &SeedCard, origin: &str) -> Value {
    json!({
        "inventory_price": card.inventory_price,
        "market_price": card.market_price,
        "card_name": card.name,
        "set_name": card.set_name,
        "card_text": card.text,
        "set_id": "OP-99",
        "rarity": card.rarity,
        "card_set_id": card.card_code,
        "card_color": card.color,
        "card_type": card.card_type,
        "life": card.life,
        "card_cost": card.cost,
        "card_power": card.power,
        "sub_types": card.sub_types,
        "counter_amount": card.counter,
        "attribute": card.attribute,
        "date_scraped": (Utc::now() - Duration::days(card.observed_days_ago)).format("%Y-%m-%d").to_string(),
        "card_image_id": card.image_id,
        "card_image": format!("{origin}media/static/Card_Images/{}.jpg", card.file()),
    })
}

fn log(path: &str, status: StatusCode) {
    eprintln!("OPTCG stand-in: GET {path} -> {}", status.as_u16());
}

async fn card(State(origin): State<String>, Path(code_id): Path<String>) -> impl IntoResponse {
    let path = format!("/api/sets/card/{code_id}/");
    let (status, body) = if code_id == "OP99-503" {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            json!({"error": "synthetic outage"}),
        )
    } else {
        let variants: Vec<Value> = catalogue()
            .iter()
            .filter(|card| card.card_code == code_id)
            .map(|card| wire(card, &origin))
            .collect();
        if variants.is_empty() {
            (StatusCode::NOT_FOUND, json!({"error": "Card not found."}))
        } else {
            (StatusCode::OK, Value::Array(variants))
        }
    };
    log(&path, status);
    (status, Json(body))
}

fn picture(file: &str) -> Vec<u8> {
    let hue = file.bytes().fold(7_u32, |hash, byte| {
        hash.wrapping_mul(31).wrapping_add(u32::from(byte))
    }) % 200;
    let tone = |base: u32| u8::try_from((base + hue).min(255)).unwrap_or(255);
    let (width, height) = (600_u32, 838_u32);
    let image = RgbImage::from_fn(width, height, |x, y| {
        let frame = x < 24 || y < 24 || x >= width - 24 || y >= height - 24;
        let band = (48..130).contains(&y);
        let art = (150..560).contains(&y) && (48..width - 48).contains(&x);
        if frame {
            Rgb([30, 30, 36])
        } else if band {
            Rgb([235, 228, 210])
        } else if art {
            let fade = u8::try_from(y / 6).unwrap_or(255);
            Rgb([tone(40), tone(70).saturating_add(fade / 4), tone(120)])
        } else {
            Rgb([tone(20), tone(20), tone(30)])
        }
    });
    let mut bytes = Vec::new();
    PngEncoder::new(&mut bytes)
        .write_image(
            image.as_raw(),
            width,
            height,
            image::ExtendedColorType::Rgb8,
        )
        .expect("a PNG encodes into memory");
    bytes
}

async fn image(Path(file): Path<String>) -> impl IntoResponse {
    let path = format!("/media/static/Card_Images/{file}");
    let id = file.strip_suffix(".jpg").unwrap_or("");
    let known = catalogue().iter().any(|card| card.file() == id);
    let status = if known {
        StatusCode::OK
    } else {
        StatusCode::NOT_FOUND
    };
    log(&path, status);
    if known {
        Ok((
            [(axum::http::header::CONTENT_TYPE, "image/jpeg")],
            picture(id),
        ))
    } else {
        Err(status)
    }
}

/// The catalogue client for a loopback stand-in. Never the live service: it is one volunteer's
/// server.
pub async fn optcg() -> anyhow::Result<Optcg> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let url = format!("http://{address}/");
    let provider = Router::new()
        .route("/api/sets/card/{code}/", get(card))
        .route("/media/static/Card_Images/{file}", get(image))
        .with_state(url.clone());
    tokio::spawn(async move { axum::serve(listener, provider).await });
    // Without the scheme: test harnesses take the first `http://127.0.0.1:<port>` they see as Axum's URL.
    eprintln!(
        "OPTCG: loopback catalogue at {address} (invented OP99 cards; OP99-503 simulates an outage)"
    );
    Optcg::new(url.parse()?)
}
