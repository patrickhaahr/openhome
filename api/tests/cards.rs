mod common;

use axum::{
    Router,
    body::Body,
    http::{Method, Request, StatusCode, header},
};
use common::{send_request_with_method, test_app_with_db};
use image::{ImageEncoder, Rgb, RgbImage, codecs::png::PngEncoder};
use openhome_api::{
    auth::{ApiKey, auth_middleware},
    routes,
    services::optcg::Optcg,
};
use serde_json::{Value, json};
use tower::ServiceExt;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};

const CARD_PATH: &str = "/api/sets/card/OP01-001/";

async fn app(server: &MockServer) -> Router {
    let (_, state) = test_app_with_db().await;
    routes::cards::router(Optcg::new(server.uri().parse().unwrap()).unwrap())
        .with_state(state)
        .layer(axum::middleware::from_fn(|request, next| {
            auth_middleware(request, next, ApiKey::new("test-api-key".into()))
        }))
}

async fn get(app: &Router, uri: &str) -> (StatusCode, Value) {
    send_request_with_method(app.clone(), uri, Method::GET, None, Some("test-api-key")).await
}

/// A flat red PNG, which the catalogue serves under a `.jpg` name.
fn png(width: u32, height: u32) -> Vec<u8> {
    let pixels = RgbImage::from_pixel(width, height, Rgb([200, 30, 30]));
    let mut bytes = Vec::new();
    PngEncoder::new(&mut bytes)
        .write_image(
            pixels.as_raw(),
            width,
            height,
            image::ExtendedColorType::Rgb8,
        )
        .unwrap();
    bytes
}

fn variant(image_id: &str, name: &str, price: Option<f64>, date: &str) -> Value {
    json!({
        "inventory_price": 1.95, "market_price": price, "card_name": name,
        "set_name": "Synthetic Set", "card_text": "[Your Turn] Gain +1000 power.",
        "set_id": "OP-01", "rarity": "L", "card_set_id": "OP01-001", "card_color": "Red",
        "card_type": "Leader", "life": "5", "card_cost": null, "card_power": "5000",
        "sub_types": "Invented Crew", "counter_amount": null, "attribute": "Slash",
        "date_scraped": date, "card_image_id": image_id,
        "card_image": format!("https://catalogue.invalid/{image_id}.jpg"),
    })
}

async fn mount_card(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path(CARD_PATH))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            variant(
                "OP01-001",
                "Invented Captain (001)",
                Some(2.25),
                "2026-11-12"
            ),
            variant(
                "OP01-001_p1",
                "Invented Captain (001) (Parallel)",
                None,
                "2026-06-09"
            ),
        ])))
        .mount(server)
        .await;
    for id in ["OP01-001", "OP01-001_p1"] {
        Mock::given(method("GET"))
            .and(path(format!("/media/static/Card_Images/{id}.jpg")))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "image/jpeg")
                    .set_body_bytes(png(60, 84)),
            )
            .mount(server)
            .await;
    }
}

async fn requests(server: &MockServer) -> usize {
    server.received_requests().await.unwrap().len()
}

#[tokio::test]
async fn first_lookup_stores_every_variant_and_later_lookups_ask_nobody() {
    let server = MockServer::start().await;
    mount_card(&server).await;
    let app = app(&server).await;

    let (status, found) = get(&app, "/api/binder/cards?card_code=op01-001").await;
    assert_eq!(status, StatusCode::OK, "{found}");
    assert_eq!(found["card_code"], "OP01-001");
    let cards = found["cards"].as_array().unwrap();
    assert_eq!(cards.len(), 2);
    let normal = &cards[0];
    assert_eq!(normal["id"], "OP01-001");
    assert_eq!(normal["name"], "Invented Captain (001)");
    assert_eq!(normal["set_name"], "Synthetic Set");
    assert_eq!(normal["rarity"], "L");
    assert_eq!(normal["life"], "5");
    assert_eq!(normal["cost"], Value::Null);
    assert_eq!(normal["power"], "5000");
    assert_eq!(normal["attribute"], "Slash");
    assert_eq!(normal["sub_types"], "Invented Crew");
    assert_eq!(normal["text"], "[Your Turn] Gain +1000 power.");
    assert_eq!(
        normal["card_value"],
        json!({"usd": 2.25, "observed_on": "2026-11-12"})
    );
    assert_eq!(normal["image_path"], "/api/binder/cards/OP01-001/image");
    assert_eq!(cards[1]["id"], "OP01-001_p1");
    assert_eq!(cards[1]["card_value"], Value::Null);
    // The card and its two images.
    assert_eq!(requests(&server).await, 3);

    let (status, again) = get(&app, "/api/binder/cards?card_code=OP01-001").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(again, found);
    assert_eq!(requests(&server).await, 3);
}

#[tokio::test]
async fn stored_images_are_full_size_avif_behind_authentication() {
    let server = MockServer::start().await;
    mount_card(&server).await;
    let app = app(&server).await;
    get(&app, "/api/binder/cards?card_code=OP01-001").await;

    let fetch = |uri: &'static str, key: Option<&'static str>| {
        let app = app.clone();
        async move {
            let mut request = Request::builder().uri(uri);
            if let Some(key) = key {
                request = request.header(header::AUTHORIZATION, format!("Bearer {key}"));
            }
            app.oneshot(request.body(Body::empty()).unwrap())
                .await
                .unwrap()
        }
    };
    let response = fetch("/api/binder/cards/OP01-001_p1/image", Some("test-api-key")).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CONTENT_TYPE], "image/avif");
    let bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    assert!(libavif::is_avif(&bytes));
    let pixels = libavif::decode_rgb(&bytes).unwrap();
    assert_eq!((pixels.width(), pixels.height()), (60, 84));
    let (red, green, blue, _) = pixels.pixel(30, 40);
    assert!(red > 170 && green < 70 && blue < 70, "{red} {green} {blue}");

    let response = fetch("/api/binder/cards/OP01-001/image", None).await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let response = fetch("/api/binder/cards/OP01-002/image", Some("test-api-key")).await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let response = fetch("/api/binder/cards/not-a-card/image", Some("test-api-key")).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn a_code_the_catalogue_does_not_list_is_empty_and_malformed_codes_are_rejected() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/sets/card/OP99-999/"))
        .respond_with(ResponseTemplate::new(404).set_body_json(json!({"error": "Card not found."})))
        .mount(&server)
        .await;
    let app = app(&server).await;

    let (status, found) = get(&app, "/api/binder/cards?card_code=OP99-999").await;
    assert_eq!(status, StatusCode::OK, "{found}");
    assert_eq!(found, json!({"card_code": "OP99-999", "cards": []}));
    assert_eq!(requests(&server).await, 1);

    for query in [
        "card_code=OP01",
        "card_code=OP01-0001",
        "card_code=..%2F..%2Fadmin",
        "card_code=OP01-001%3Fx%3D1",
        "card_code=",
        "",
    ] {
        let (status, body) = get(&app, &format!("/api/binder/cards?{query}")).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{query}: {body}");
    }
    assert_eq!(requests(&server).await, 1);
}

#[tokio::test]
async fn every_catalogue_failure_is_a_sanitised_503_that_stores_nothing() {
    let leak = "synthetic-upstream-secret-text";
    let oversized_image = vec![0x89; 3 * 1024 * 1024];
    let wrong_code = json!([variant("OP02-002", leak, Some(1.0), "2026-11-12")]);
    let priced_without_date = json!([{
        "card_set_id": "OP01-001", "card_image_id": "OP01-001", "card_name": leak, "market_price": 1.0,
    }]);
    let failures: Vec<(&str, ResponseTemplate, ResponseTemplate)> = vec![
        (
            "server error",
            ResponseTemplate::new(500).set_body_string(leak),
            ResponseTemplate::new(200),
        ),
        (
            "rate limit",
            ResponseTemplate::new(429).set_body_string(leak),
            ResponseTemplate::new(200),
        ),
        (
            "not JSON",
            ResponseTemplate::new(200).set_body_string(leak),
            ResponseTemplate::new(200),
        ),
        (
            "redirect",
            ResponseTemplate::new(302).insert_header("location", "/elsewhere"),
            ResponseTemplate::new(200),
        ),
        (
            "slow",
            ResponseTemplate::new(200)
                .set_delay(std::time::Duration::from_secs(8))
                .set_body_json(json!([])),
            ResponseTemplate::new(200),
        ),
        (
            "another code",
            ResponseTemplate::new(200).set_body_json(wrong_code),
            ResponseTemplate::new(200).set_body_bytes(png(8, 8)),
        ),
        (
            "price without date",
            ResponseTemplate::new(200).set_body_json(priced_without_date),
            ResponseTemplate::new(200).set_body_bytes(png(8, 8)),
        ),
        (
            "image refused",
            ResponseTemplate::new(200).set_body_json(json!([variant(
                "OP01-001",
                "A",
                Some(1.0),
                "2026-11-12"
            )])),
            ResponseTemplate::new(500).set_body_string(leak),
        ),
        (
            "image is not a picture",
            ResponseTemplate::new(200).set_body_json(json!([variant(
                "OP01-001",
                "A",
                Some(1.0),
                "2026-11-12"
            )])),
            ResponseTemplate::new(200).set_body_string(leak),
        ),
        (
            "image too large",
            ResponseTemplate::new(200).set_body_json(json!([variant(
                "OP01-001",
                "A",
                Some(1.0),
                "2026-11-12"
            )])),
            ResponseTemplate::new(200).set_body_bytes(oversized_image),
        ),
    ];
    for (name, card, image) in failures {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path(CARD_PATH))
            .respond_with(card)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/media/static/Card_Images/OP01-001.jpg"))
            .respond_with(image)
            .mount(&server)
            .await;
        let app = app(&server).await;

        let (status, body) = get(&app, "/api/binder/cards?card_code=OP01-001").await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{name}: {body}");
        assert_eq!(body["code"], "card_data_unavailable", "{name}");
        assert!(!body.to_string().contains(leak), "{name}: {body}");
        assert!(
            server
                .received_requests()
                .await
                .unwrap()
                .iter()
                .all(|request| request.url.path() != "/elsewhere"),
            "{name} followed a redirect"
        );

        // Nothing was stored: once the catalogue recovers, the code is asked for again.
        server.reset().await;
        mount_card(&server).await;
        let (status, found) = get(&app, "/api/binder/cards?card_code=OP01-001").await;
        assert_eq!(status, StatusCode::OK, "{name}: {found}");
        assert_eq!(found["cards"].as_array().unwrap().len(), 2, "{name}");
        assert_eq!(requests(&server).await, 3, "{name}");
    }
}

#[tokio::test]
async fn an_unreachable_catalogue_is_unavailable() {
    let closed = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}/", closed.local_addr().unwrap());
    drop(closed);
    let (_, state) = test_app_with_db().await;
    let app = routes::cards::router(Optcg::new(origin.parse().unwrap()).unwrap())
        .with_state(state)
        .layer(axum::middleware::from_fn(|request, next| {
            auth_middleware(request, next, ApiKey::new("test-api-key".into()))
        }));
    let (status, body) = get(&app, "/api/binder/cards?card_code=OP01-001").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert_eq!(body["code"], "card_data_unavailable");
}
