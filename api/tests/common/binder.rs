//! Binder test fixtures: a synthetic OPTCG catalogue and Binder request helpers.
#![allow(dead_code)]

use axum::{
    Router,
    http::{Method, StatusCode},
};
use image::{ImageEncoder, Rgb, RgbImage, codecs::png::PngEncoder};
use openhome_api::{
    AppState,
    auth::{ApiKey, auth_middleware},
    routes,
    services::{
        card_value::{DailyRates, Rate},
        exchange_rates,
        optcg::Optcg,
        training::CalendarDay,
    },
};
use serde_json::{Value, json};
use wiremock::{
    Mock, MockServer, Request, ResponseTemplate,
    matchers::{method, path_regex},
};

use super::{send_request_with_method, test_app_with_db};

pub const LISTED: [(&str, &[&str]); 7] = [
    ("OP01-001", &["OP01-001", "OP01-001_p1"]),
    ("OP01-002", &["OP01-002"]),
    ("OP01-010", &["OP01-010", "OP01-010_p1", "OP01-010_p2"]),
    ("OP09-034", &["EB03_OP09-034_p1", "OP09-034"]),
    ("OP10-001", &["OP10-001"]),
    ("ST01-001", &["ST01-001"]),
    ("OP02-001", &["OP02-001"]),
];
pub const OUTAGE: &str = "OP77-503";

/// The catalogue's price for a code and how many days ago it observed it. Most cards are priced
/// at 2.25 yesterday; the rest probe rate fallback, the stale boundary and unpriced cards.
pub fn market_price(code: &str) -> (Option<f64>, u64) {
    match code {
        "OP01-002" => (None, 1),
        "OP01-010" => (Some(2.25), 20),
        "OP10-001" => (Some(7.10), 30),
        "ST01-001" => (Some(3.33), 31),
        "OP02-001" => (Some(4.00), 120),
        _ => (Some(2.25), 1),
    }
}

pub fn days_ago(days: u64) -> CalendarDay {
    CalendarDay::of(chrono::Utc::now())
        .checked_sub_days(days)
        .unwrap()
}

/// Three ECB days: yesterday's, twenty days ago and forty-five days ago.
pub async fn store_rates(state: &AppState) {
    for (days, usd, dkk) in [
        (1, "1.1081", "7.4754"),
        (20, "1.2000", "7.5000"),
        (45, "1.1000", "7.4000"),
    ] {
        exchange_rates::store(
            &state.db,
            &[DailyRates {
                date: days_ago(days),
                usd: Rate::parse(usd).unwrap(),
                dkk: Rate::parse(dkk).unwrap(),
            }],
        )
        .await
        .unwrap();
    }
}

pub fn png() -> Vec<u8> {
    let pixels = RgbImage::from_pixel(60, 84, Rgb([200, 30, 30]));
    let mut bytes = Vec::new();
    PngEncoder::new(&mut bytes)
        .write_image(pixels.as_raw(), 60, 84, image::ExtendedColorType::Rgb8)
        .unwrap();
    bytes
}

pub async fn catalogue() -> MockServer {
    let server = MockServer::start().await;
    let origin = server.uri();
    Mock::given(method("GET"))
        .and(path_regex(r"^/api/sets/card/[^/]+/$"))
        .respond_with(move |request: &Request| {
            let code = request.url.path().split('/').nth(4).unwrap();
            if code == OUTAGE {
                return ResponseTemplate::new(503);
            }
            let Some((_, ids)) = LISTED.iter().find(|(listed, _)| *listed == code) else {
                return ResponseTemplate::new(404)
                    .set_body_json(json!({"error": "Card not found."}));
            };
            let (price, age) = market_price(code);
            let observed = days_ago(age).to_string();
            ResponseTemplate::new(200).set_body_json(
                ids.iter()
                    .map(|id| {
                        json!({
                            "inventory_price": 1.95, "market_price": price,
                            "card_name": format!("Invented {id}"), "set_name": "Synthetic Set",
                            "card_text": "[Your Turn] Gain +1000 power.", "set_id": "OP-01",
                            "rarity": "L", "card_set_id": code, "card_color": "Red",
                            "card_type": "Leader", "life": "5", "card_cost": null,
                            "card_power": "5000", "sub_types": "Invented Crew",
                            "counter_amount": null, "attribute": "Slash",
                            "date_scraped": observed, "card_image_id": id,
                            "card_image": format!("{origin}/media/static/Card_Images/{id}.jpg"),
                        })
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path_regex(r"^/media/static/Card_Images/[^/]+\.jpg$"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(png()))
        .mount(&server)
        .await;
    server
}

pub async fn catalogue_requests(server: &MockServer) -> usize {
    server.received_requests().await.unwrap().len()
}

pub async fn app(server: &MockServer) -> (Router, AppState) {
    let (_, state) = test_app_with_db().await;
    let catalogue = Optcg::new(server.uri().parse().unwrap()).unwrap();
    let app = routes::wishlist::router(None)
        .merge(routes::cards::router(catalogue.clone()))
        .merge(routes::binder::router(catalogue))
        .with_state(state.clone())
        .layer(axum::middleware::from_fn(|request, next| {
            auth_middleware(request, next, ApiKey::new("test-api-key".into()))
        }));
    (app, state)
}

pub async fn send(
    app: &Router,
    method: Method,
    uri: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    send_request_with_method(app.clone(), uri, method, body, Some("test-api-key")).await
}

pub async fn owner(app: &Router, name: &str) -> String {
    let (status, body) = send(
        app,
        Method::POST,
        "/api/household-users",
        Some(json!({"name": name})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body["id"].as_str().unwrap().to_owned()
}

pub async fn add(app: &Router, body: Value) -> (StatusCode, Value) {
    send(app, Method::POST, "/api/binder/entries", Some(body)).await
}

/// The whole list response: `entries` and each owner's `binder_values`.
pub async fn list_response(app: &Router, query: &str) -> Value {
    let (status, body) = send(
        app,
        Method::GET,
        &format!("/api/binder/entries{query}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body
}

pub async fn list(app: &Router, query: &str) -> Vec<Value> {
    list_response(app, query).await["entries"]
        .as_array()
        .unwrap()
        .clone()
}

pub fn ids(entries: &[Value]) -> Vec<(&str, &str, i64)> {
    entries
        .iter()
        .map(|entry| {
            (
                entry["card"]["id"].as_str().unwrap(),
                entry["language"].as_str().unwrap(),
                entry["quantity"].as_i64().unwrap(),
            )
        })
        .collect()
}
