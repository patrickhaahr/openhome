//! The first pass on a store whose rates do not reach back to the oldest stored Card Price reads
//! the ECB's full history, so old prices are valued at their own day's rate.
mod common;

use common::{binder::*, ecb};
use openhome_api::services::exchange_rates::{self, Pass};
use serde_json::{Value, json};
use wiremock::{MockServer, ResponseTemplate};

fn day(days: u64) -> String {
    days_ago(days).to_string()
}

/// A Card the catalogue priced 120 days ago, held by Ada.
async fn ada_with_an_old_card(app: &axum::Router) -> Value {
    let ada = owner(app, "Ada").await;
    let (status, body) = add(
        app,
        json!({"owner_id": ada, "card_id": "OP02-001", "request_id": "r1"}),
    )
    .await;
    assert!(status.is_success(), "{body}");
    list_response(app, "").await
}

async fn stored_days(state: &openhome_api::AppState) -> Vec<String> {
    sqlx::query_scalar("SELECT rate_date FROM exchange_rates ORDER BY rate_date")
        .fetch_all(&state.db)
        .await
        .unwrap()
}

fn history() -> String {
    ecb::history_file(&[
        (&day(1), "1.1081", Some("7.4754")),
        (&day(20), "1.2000", Some("7.5000")),
        (&day(100), "1.1000", Some("7.4000")),
        (&day(122), "1.0700", None),
        (&day(125), "1.0500", Some("7.4400")),
    ])
}

async fn serving_history(history: String) -> MockServer {
    let server = ecb::serving(ecb::file(&day(1), "1.1081", "7.4754")).await;
    ecb::serve_history(&server, ResponseTemplate::new(200).set_body_string(history)).await;
    server
}

async fn requests_to(server: &MockServer, path: &str) -> usize {
    let requests = server.received_requests().await.unwrap();
    requests.iter().filter(|r| r.url.path() == path).count()
}

fn priced_old_card() -> Value {
    json!({"usd": 4.0, "eur": 3.81, "dkk": 28.34, "observed_on": day(120),
           "rate_date": day(125), "stale": true})
}

#[tokio::test]
async fn a_first_deployment_backfills_history_and_old_prices_count_as_stale_value() {
    let catalogue = catalogue().await;
    let (app, state) = app(&catalogue).await;
    let rates = serving_history(history()).await;
    let ecb = ecb::client(&rates);

    let before = ada_with_an_old_card(&app).await;
    assert_eq!(
        before["entries"][0]["card"]["value_unavailable"],
        "no_exchange_rates"
    );
    assert_eq!(before["binder_values"][0]["unpriced_entries"], 1);

    let pass = exchange_rates::refresh(&state.db, &ecb).await.unwrap();
    assert!(matches!(pass, Pass::History { days: 4 }), "{pass:?}");
    assert_eq!(
        stored_days(&state).await,
        [day(125), day(100), day(20), day(1)],
        "the day without a DKK rate is skipped, the rest stored"
    );

    let after = list_response(&app, "").await;
    assert_eq!(after["entries"][0]["card"]["card_value"], priced_old_card());
    let total = &after["binder_values"][0];
    assert_eq!(
        (&total["usd"], &total["eur"], &total["dkk"]),
        (&json!(4.0), &json!(3.81), &json!(28.34))
    );
    assert_eq!(total["unpriced_entries"], 0);
    assert_eq!(total["oldest_observed_on"], day(120));

    let pass = exchange_rates::refresh(&state.db, &ecb).await.unwrap();
    assert!(matches!(pass, Pass::Daily(_)), "{pass:?}");
    assert_eq!(requests_to(&rates, ecb::HISTORY_PATH).await, 1);
    assert_eq!(requests_to(&rates, ecb::FILE_PATH).await, 1);
    assert_eq!(list_response(&app, "").await, after);
}

#[tokio::test]
async fn rates_that_start_after_the_oldest_price_are_backfilled_too() {
    let catalogue = catalogue().await;
    let (app, state) = app(&catalogue).await;
    ecb::store_day(&state.db, &day(3), "1.2000", "7.5000").await;
    let rates = serving_history(history()).await;
    let ecb = ecb::client(&rates);
    let before = ada_with_an_old_card(&app).await;
    assert_eq!(before["binder_values"][0]["unpriced_entries"], 1);

    let pass = exchange_rates::refresh(&state.db, &ecb).await.unwrap();
    assert!(matches!(pass, Pass::History { .. }), "{pass:?}");
    assert_eq!(
        list_response(&app, "").await["entries"][0]["card"]["card_value"],
        priced_old_card()
    );
}

#[tokio::test]
async fn a_failed_history_read_writes_nothing_and_does_not_fall_back_to_the_daily_file() {
    let catalogue = catalogue().await;
    let (app, state) = app(&catalogue).await;
    let rates = serving_history(history()).await;
    let ecb = ecb::client(&rates);
    let before = ada_with_an_old_card(&app).await;

    let valid = history();
    let failures = [
        (
            "outage",
            ResponseTemplate::new(503).set_body_string(valid.clone()),
        ),
        (
            "not found",
            ResponseTemplate::new(404).set_body_string(valid.clone()),
        ),
        (
            "bad rate on the oldest day",
            ResponseTemplate::new(200).set_body_string(ecb::history_file(&[
                (&day(1), "1.1081", Some("7.4754")),
                (&day(125), "free", Some("7.4400")),
            ])),
        ),
        (
            "truncated",
            ResponseTemplate::new(200).set_body_string(valid.replace("</gesmes:Envelope>", "")),
        ),
        (
            "no complete day",
            ResponseTemplate::new(200).set_body_string(ecb::history_file(&[(
                &day(1),
                "1.1081",
                None,
            )])),
        ),
        (
            "oversized",
            ResponseTemplate::new(200)
                .set_body_string(format!("{valid}{}", " ".repeat(16 * 1024 * 1024))),
        ),
        (
            "redirect",
            ResponseTemplate::new(302).insert_header("location", "/elsewhere"),
        ),
    ];
    for (name, response) in failures {
        ecb::serve(
            &rates,
            ResponseTemplate::new(200).set_body_string(ecb::file(&day(1), "1.1081", "7.4754")),
        )
        .await;
        ecb::serve_history(&rates, response).await;
        ecb::also(
            &rates,
            "/elsewhere",
            ResponseTemplate::new(200).set_body_string(valid.clone()),
        )
        .await;
        assert!(
            exchange_rates::refresh(&state.db, &ecb).await.is_err(),
            "{name} was accepted"
        );
        assert!(stored_days(&state).await.is_empty(), "{name} stored rates");
        assert_eq!(
            list_response(&app, "").await,
            before,
            "{name} changed values"
        );
    }
}
