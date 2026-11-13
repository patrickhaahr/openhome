mod common;

use axum::http::StatusCode;
use common::binder::*;
use serde_json::{Value, json};

fn day(days: u64) -> String {
    days_ago(days).to_string()
}

async fn add_to(app: &axum::Router, owner: &str, card: &str, quantity: i64, language: &str) {
    let (status, body) = add(
        app,
        json!({"owner_id": owner, "card_id": card, "quantity": quantity, "language": language,
               "request_id": format!("{owner}-{card}-{language}")}),
    )
    .await;
    assert!(
        status == StatusCode::CREATED || status == StatusCode::OK,
        "{body}"
    );
}

fn by_card<'a>(entries: &'a [Value], id: &str, language: &str) -> &'a Value {
    entries
        .iter()
        .find(|entry| entry["card"]["id"] == id && entry["language"] == language)
        .unwrap()
}

#[tokio::test]
async fn a_card_value_is_the_price_in_three_currencies_at_the_rate_of_its_own_day() {
    let server = catalogue().await;
    let (app, state) = app(&server).await;
    store_rates(&state).await;
    let ada = owner(&app, "Ada").await;
    for card in ["OP01-001", "OP01-010", "OP10-001", "ST01-001", "OP01-002"] {
        add_to(&app, &ada, card, 1, "en").await;
    }
    let entries = list(&app, "").await;

    for (id, value) in [
        (
            "OP01-001",
            json!({"usd": 2.25, "eur": 2.03, "dkk": 15.18, "observed_on": day(1),
                   "rate_date": day(1), "stale": false}),
        ),
        (
            "OP01-010",
            json!({"usd": 2.25, "eur": 1.88, "dkk": 14.06, "observed_on": day(20),
                   "rate_date": day(20), "stale": false}),
        ),
        (
            "OP10-001",
            json!({"usd": 7.10, "eur": 6.45, "dkk": 47.76, "observed_on": day(30),
                   "rate_date": day(45), "stale": false}),
        ),
        (
            "ST01-001",
            json!({"usd": 3.33, "eur": 3.03, "dkk": 22.40, "observed_on": day(31),
                   "rate_date": day(45), "stale": true}),
        ),
    ] {
        let entry = by_card(&entries, id, "en");
        assert_eq!(entry["card"]["card_value"], value, "{id}");
        assert_eq!(entry["card"]["value_unavailable"], Value::Null, "{id}");
        let (_, searched) = send(
            &app,
            axum::http::Method::GET,
            &format!(
                "/api/binder/cards?card_code={}",
                id.split('_').next().unwrap()
            ),
            None,
        )
        .await;
        assert_eq!(
            searched["cards"][0]["card_value"], value,
            "a Card read matches: {id}"
        );
    }
    let unpriced = by_card(&entries, "OP01-002", "en");
    assert_eq!(unpriced["card"]["card_value"], Value::Null);
    assert_eq!(unpriced["card"]["value_unavailable"], "no_catalogue_price");
}

#[tokio::test]
async fn without_rates_at_or_before_the_price_day_there_is_no_value_and_a_reason() {
    let server = catalogue().await;
    let (app, state) = app(&server).await;
    let ada = owner(&app, "Ada").await;
    add_to(&app, &ada, "OP01-001", 2, "en").await;
    let read = list_response(&app, "").await;
    let card = &read["entries"][0]["card"];
    assert_eq!(card["card_value"], Value::Null);
    assert_eq!(card["value_unavailable"], "no_exchange_rates");
    assert_eq!(
        read["binder_values"],
        json!([{"owner_id": ada, "usd": 0.0, "eur": 0.0, "dkk": 0.0, "unpriced_entries": 1,
                "oldest_observed_on": null}])
    );

    store_rates(&state).await;
    assert_eq!(
        list(&app, "").await[0]["card"]["card_value"]["rate_date"],
        json!(day(1))
    );
}

#[tokio::test]
async fn a_binder_value_sums_quantity_times_card_value_and_counts_what_has_no_price() {
    let server = catalogue().await;
    let (app, state) = app(&server).await;
    store_rates(&state).await;
    let ada = owner(&app, "Ada").await;
    let bo = owner(&app, "Bo").await;
    let cy = owner(&app, "Cy").await;
    add_to(&app, &ada, "OP01-001", 2, "en").await;
    add_to(&app, &ada, "ST01-001", 3, "en").await;
    add_to(&app, &ada, "OP01-002", 1, "en").await;
    add_to(&app, &ada, "OP01-001", 4, "ja").await;
    add_to(&app, &bo, "OP01-001", 1, "en").await;

    let read = list_response(&app, "").await;
    assert_eq!(
        read["binder_values"],
        json!([
            {"owner_id": ada, "usd": 14.49, "eur": 13.15, "dkk": 97.56, "unpriced_entries": 2,
             "oldest_observed_on": day(31)},
            {"owner_id": bo, "usd": 2.25, "eur": 2.03, "dkk": 15.18, "unpriced_entries": 0,
             "oldest_observed_on": day(1)},
            {"owner_id": cy, "usd": 0.0, "eur": 0.0, "dkk": 0.0, "unpriced_entries": 0,
             "oldest_observed_on": null},
        ]),
        "stale prices count, Japanese and unpriced entries do not, and an empty Binder is worth zero"
    );
    let ja = by_card(read["entries"].as_array().unwrap(), "OP01-001", "ja");
    assert_eq!(ja["card"]["card_value"], Value::Null);
    assert_eq!(ja["card"]["value_unavailable"], "japanese_printing");

    let only_bo = list_response(&app, &format!("?owner_id={bo}")).await;
    assert_eq!(only_bo["binder_values"].as_array().unwrap().len(), 1);
    assert_eq!(only_bo["binder_values"][0]["owner_id"], json!(bo));
    let only_cy = list_response(&app, &format!("?owner_id={cy}")).await;
    assert_eq!(only_cy["entries"], json!([]));
    assert_eq!(only_cy["binder_values"][0]["owner_id"], json!(cy));
    assert_eq!(only_cy["binder_values"][0]["dkk"], json!(0.0));
    let nobody = list_response(&app, "?owner_id=nobody").await;
    assert_eq!(nobody, json!({"entries": [], "binder_values": []}));
}
