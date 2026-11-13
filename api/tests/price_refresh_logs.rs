mod common;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::http::StatusCode;
use common::binder::*;
use openhome_api::services::{optcg::Optcg, price_refresh};
use serde_json::{Value, json};
use wiremock::{Mock, MockServer, ResponseTemplate, matchers::path};

#[derive(Clone, Default)]
struct Captured(Arc<Mutex<Vec<u8>>>);

impl std::io::Write for Captured {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

const SECRET: &str = "upstream-secret";

fn variant(origin: &str, overrides: &Value) -> Value {
    let mut card = json!({
        "market_price": 2.25, "card_name": "Invented", "set_name": "Synthetic Set",
        "card_text": "text", "set_id": "OP-01", "rarity": "L", "card_set_id": "OP01-001",
        "card_color": "Red", "card_type": "Leader", "life": "5", "card_cost": null,
        "card_power": "5000", "sub_types": "Crew", "counter_amount": null,
        "attribute": "Slash", "date_scraped": days_ago(0).to_string(),
        "card_image_id": "OP01-001",
        "card_image": format!("{origin}/media/static/Card_Images/OP01-001.jpg"),
    });
    for (key, value) in overrides.as_object().unwrap() {
        card[key] = value.clone();
    }
    card
}

#[tokio::test]
async fn a_failing_catalogue_answer_never_puts_its_text_in_the_logs() {
    let server = catalogue().await;
    let (app, state) = app(&server).await;
    let ada = owner(&app, "Ada").await;
    let (status, body) = add(
        &app,
        json!({"owner_id": ada, "card_id": "OP01-001", "request_id": "r"}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let origin = server.uri();
    let answers = [
        ResponseTemplate::new(200)
            .set_body_json([variant(&origin, &json!({"market_price": SECRET}))]),
        ResponseTemplate::new(200)
            .set_body_json([variant(&origin, &json!({"card_name": {"x": SECRET}}))]),
        ResponseTemplate::new(200).set_body_json([variant(
            &origin,
            &json!({"card_name": format!("{SECRET}\u{7}")}),
        )]),
        ResponseTemplate::new(200)
            .set_body_json([variant(&origin, &json!({"card_set_id": SECRET}))]),
        ResponseTemplate::new(200).set_body_json([variant(
            &origin,
            &json!({"card_image": format!("https://{SECRET}.example/x.jpg")}),
        )]),
        ResponseTemplate::new(200)
            .set_body_json([variant(&origin, &json!({"date_scraped": SECRET}))]),
        ResponseTemplate::new(200).set_body_string(SECRET),
        ResponseTemplate::new(200).set_body_json(json!({"error": SECRET})),
        ResponseTemplate::new(404).set_body_json(json!({"error": SECRET})),
        ResponseTemplate::new(500).set_body_string(SECRET),
    ];

    let logs = Captured::default();
    let _capture = tracing::subscriber::set_default(
        tracing_subscriber::fmt()
            .with_writer({
                let logs = logs.clone();
                move || logs.clone()
            })
            .with_ansi(false)
            .finish(),
    );
    let catalogue = Optcg::new(origin.parse().unwrap()).unwrap();
    for answer in answers {
        mount_once(&server, answer).await;
        let pass = price_refresh::refresh(&state.db, &catalogue, Duration::ZERO)
            .await
            .unwrap();
        assert_eq!(pass.failed, 1);
    }

    let logs = String::from_utf8(logs.0.lock().unwrap().clone()).unwrap();
    assert!(
        logs.contains("OP01-001"),
        "the failure is still logged: {logs}"
    );
    assert!(!logs.contains(SECRET), "{logs}");
}

async fn mount_once(server: &MockServer, answer: ResponseTemplate) {
    Mock::given(path("/api/sets/card/OP01-001/"))
        .respond_with(answer)
        .up_to_n_times(1)
        .with_priority(1)
        .mount(server)
        .await;
}
