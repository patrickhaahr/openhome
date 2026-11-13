mod common;

use std::time::Duration;

use common::{binder::*, ecb};
use openhome_api::services::exchange_rates::{self, Pass};
use serde_json::{Value, json};
use wiremock::ResponseTemplate;

async fn ada_with(app: &axum::Router, cards: &[&str]) -> Vec<Value> {
    let ada = owner(app, "Ada").await;
    for (index, card) in cards.iter().enumerate() {
        let (status, body) = add(
            app,
            json!({"owner_id": ada, "card_id": card, "request_id": format!("r{index}")}),
        )
        .await;
        assert!(status.is_success(), "{body}");
    }
    list(app, "").await
}

#[tokio::test]
async fn a_refresh_stores_the_files_day_and_prices_convert_at_it() {
    let catalogue = catalogue().await;
    let (app, state) = app(&catalogue).await;
    ecb::store_day(&state.db, &days_ago(3).to_string(), "1.2000", "7.5000").await;
    let rates = ecb::serving(ecb::file(&days_ago(1).to_string(), "1.1081", "7.4754")).await;
    let ecb = ecb::client(&rates);

    let entries = ada_with(&app, &["OP01-001"]).await;
    assert_eq!(
        entries[0]["card"]["card_value"]["rate_date"],
        json!(days_ago(3).to_string()),
        "before the file's day, the earlier day's rates convert"
    );

    let pass = exchange_rates::refresh(&state.db, &ecb).await.unwrap();
    assert!(
        matches!(pass, Pass::Recent { days: 1 }),
        "rates reach back past every price, so the 90-day file is enough: {pass:?}"
    );
    let entries = list(&app, "").await;
    assert_eq!(
        entries[0]["card"]["card_value"],
        json!({"usd": 2.25, "eur": 2.03, "dkk": 15.18, "observed_on": days_ago(1).to_string(),
               "rate_date": days_ago(1).to_string(), "stale": false})
    );

    exchange_rates::refresh(&state.db, &ecb).await.unwrap();
    assert_eq!(
        list(&app, "").await,
        entries,
        "a repeated read of one day changes nothing"
    );
}

#[tokio::test]
async fn each_day_the_ecb_publishes_is_kept_beside_the_earlier_ones() {
    let catalogue = catalogue().await;
    let (app, state) = app(&catalogue).await;
    ecb::store_day(&state.db, &days_ago(40).to_string(), "1.1000", "7.4000").await;
    let rates = ecb::serving(ecb::file(&days_ago(20).to_string(), "1.2000", "7.5000")).await;
    let ecb = ecb::client(&rates);
    exchange_rates::refresh(&state.db, &ecb).await.unwrap();
    ecb::serve(
        &rates,
        ResponseTemplate::new(200).set_body_string(ecb::file(
            &days_ago(1).to_string(),
            "1.1081",
            "7.4754",
        )),
    )
    .await;
    exchange_rates::refresh(&state.db, &ecb).await.unwrap();

    let entries = ada_with(&app, &["OP01-001", "OP01-010"]).await;
    let (yesterday, older) = (
        &entries[0]["card"]["card_value"],
        &entries[1]["card"]["card_value"],
    );
    assert_eq!(
        (&yesterday["eur"], &yesterday["rate_date"]),
        (&json!(2.03), &json!(days_ago(1).to_string()))
    );
    assert_eq!(
        (&older["eur"], &older["dkk"], &older["rate_date"]),
        (
            &json!(1.88),
            &json!(14.06),
            &json!(days_ago(20).to_string())
        )
    );
}

#[tokio::test]
async fn a_failed_read_keeps_the_previous_rates_and_writes_nothing() {
    let catalogue = catalogue().await;
    let (app, state) = app(&catalogue).await;
    ecb::store_day(&state.db, &days_ago(30).to_string(), "1.1000", "7.4000").await;
    ecb::store_day(&state.db, &days_ago(1).to_string(), "1.1081", "7.4754").await;
    let rates = ecb::serving(ecb::file(&days_ago(1).to_string(), "1.1081", "7.4754")).await;
    let ecb = ecb::client(&rates);
    let before = ada_with(&app, &["OP01-001", "OP01-010"]).await;
    assert_eq!(before[0]["card"]["card_value"]["eur"], json!(2.03));
    assert_eq!(
        before[1]["card"]["card_value"]["rate_date"],
        json!(days_ago(30).to_string()),
        "nothing is stored for twenty days ago yet"
    );

    // Each failing answer carries a file for twenty days ago: if it were stored, that card would convert at it.
    let other_day = ecb::file(&days_ago(20).to_string(), "1.2000", "7.5000");
    let no_dkk = other_day.replace(r#"<Cube currency="DKK" rate="7.5000"/>"#, "");
    let failures = [
        (
            "outage",
            ResponseTemplate::new(503).set_body_string(other_day.clone()),
        ),
        (
            "not found",
            ResponseTemplate::new(404).set_body_string(other_day.clone()),
        ),
        (
            "not XML",
            ResponseTemplate::new(200).set_body_string("<html>maintenance"),
        ),
        ("no DKK", ResponseTemplate::new(200).set_body_string(no_dkk)),
        (
            "bad rate",
            ResponseTemplate::new(200).set_body_string(other_day.replace("7.5000", "free")),
        ),
        (
            "oversized",
            ResponseTemplate::new(200)
                .set_body_string(format!("{other_day}{}", " ".repeat(1024 * 1024))),
        ),
        (
            "redirect",
            ResponseTemplate::new(302).insert_header("location", "/elsewhere"),
        ),
        (
            "slow",
            ResponseTemplate::new(200)
                .set_body_string(other_day.clone())
                .set_delay(Duration::from_secs(8)),
        ),
    ];
    for (name, response) in failures {
        ecb::serve(&rates, response).await;
        if name == "redirect" {
            let target = ResponseTemplate::new(200).set_body_string(other_day.clone());
            ecb::also(&rates, "/elsewhere", target).await;
        }
        assert!(
            exchange_rates::refresh(&state.db, &ecb).await.is_err(),
            "{name} was accepted"
        );
        assert_eq!(
            list(&app, "").await,
            before,
            "{name} changed the stored rates"
        );
    }
}
