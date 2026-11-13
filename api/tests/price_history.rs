mod common;

use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use std::time::Duration;

use axum::http::{Method, StatusCode};
use common::binder::*;
use openhome_api::services::{
    optcg::Optcg,
    price_refresh::{self, Pass},
};
use serde_json::{Value, json};
use wiremock::{Mock, MockServer, ResponseTemplate, matchers::path};

/// A catalogue whose observations move `days` newer, each day also adding five cents.
async fn moving_catalogue() -> (MockServer, Arc<AtomicU64>) {
    let days = Arc::new(AtomicU64::new(0));
    let shift = days.clone();
    let server = catalogue_priced(move |code| {
        let (price, age) = market_price(code);
        let days = shift.load(Ordering::SeqCst);
        (
            price.map(|price| price + days as f64 * 0.05),
            age.saturating_sub(days),
        )
    })
    .await;
    (server, days)
}

async fn card_requests(server: &MockServer) -> usize {
    server
        .received_requests()
        .await
        .unwrap()
        .iter()
        .filter(|request| request.url.path().starts_with("/api/sets/card/"))
        .count()
}

async fn own(app: &axum::Router, owner: &str, card: &str) {
    let (status, body) = add(
        app,
        json!({"owner_id": owner, "card_id": card, "request_id": format!("{owner}-{card}")}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
}

async fn history(app: &axum::Router, card: &str, query: &str) -> (StatusCode, Value) {
    send(
        app,
        Method::GET,
        &format!("/api/binder/cards/{card}/prices{query}"),
        None,
    )
    .await
}

async fn points(app: &axum::Router, card: &str) -> Vec<(String, f64)> {
    let (status, body) = history(app, card, "").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body["prices"]
        .as_array()
        .unwrap()
        .iter()
        .map(|point| {
            (
                point["observed_on"].as_str().unwrap().to_owned(),
                point["usd"].as_f64().unwrap(),
            )
        })
        .collect()
}

async fn refresh(state: &openhome_api::AppState, server: &MockServer) -> Pass {
    let catalogue = Optcg::new(server.uri().parse().unwrap()).unwrap();
    price_refresh::refresh(&state.db, &catalogue, Duration::ZERO)
        .await
        .unwrap()
}

fn day(days: u64) -> String {
    days_ago(days).to_string()
}

#[tokio::test]
async fn a_stored_card_records_its_first_price_point_and_an_unpriced_one_records_none() {
    let (server, _) = moving_catalogue().await;
    let (app, _) = app(&server).await;
    let ada = owner(&app, "Ada").await;
    own(&app, &ada, "OP01-001").await;
    own(&app, &ada, "OP01-002").await;
    send(
        &app,
        Method::GET,
        "/api/binder/cards?card_code=OP10-001",
        None,
    )
    .await;

    assert_eq!(points(&app, "OP01-001").await, [(day(1), 2.25)]);
    assert_eq!(
        points(&app, "OP01-001_p1").await,
        [(day(1), 2.25)],
        "every stored variant"
    );
    assert_eq!(
        points(&app, "OP10-001").await,
        [(day(30), 7.10)],
        "a searched Card with no owner"
    );
    assert_eq!(points(&app, "OP01-002").await, []);
}

#[tokio::test]
async fn the_refresh_asks_once_per_owned_code_and_appends_only_newer_observations() {
    let (server, days) = moving_catalogue().await;
    let (app, state) = app(&server).await;
    store_rates(&state).await;
    let ada = owner(&app, "Ada").await;
    let bo = owner(&app, "Bo").await;
    own(&app, &ada, "OP01-001").await;
    own(&app, &bo, "OP01-001").await;
    own(&app, &ada, "OP01-010").await;
    send(
        &app,
        Method::GET,
        "/api/binder/cards?card_code=OP10-001",
        None,
    )
    .await;

    let asked = card_requests(&server).await;
    assert_eq!(
        refresh(&state, &server).await,
        Pass {
            codes: 2,
            appended: 0,
            failed: 0
        }
    );
    assert_eq!(
        card_requests(&server).await - asked,
        2,
        "one per owned code"
    );
    assert_eq!(points(&app, "OP01-001").await, [(day(1), 2.25)]);

    days.store(1, Ordering::SeqCst);
    assert_eq!(
        refresh(&state, &server).await,
        Pass {
            codes: 2,
            appended: 2,
            failed: 0
        }
    );
    assert_eq!(
        points(&app, "OP01-001").await,
        [(day(1), 2.25), (day(0), 2.30)]
    );
    assert_eq!(
        points(&app, "OP01-001_p1").await,
        [(day(1), 2.25)],
        "unowned variant"
    );
    assert_eq!(
        points(&app, "OP10-001").await,
        [(day(30), 7.10)],
        "unowned Card"
    );
    let entries = list(&app, "").await;
    let card = &by_id(&entries, "OP01-001")["card"];
    assert_eq!(card["card_value"]["usd"], 2.30);
    assert_eq!(card["card_value"]["observed_on"], day(0));

    assert_eq!(
        refresh(&state, &server).await,
        Pass {
            codes: 2,
            appended: 0,
            failed: 0
        }
    );
    assert_eq!(
        points(&app, "OP01-001").await.len(),
        2,
        "same date adds nothing"
    );
}

fn by_id<'a>(entries: &'a [Value], id: &str) -> &'a Value {
    entries
        .iter()
        .find(|entry| entry["card"]["id"] == id)
        .unwrap()
}

#[tokio::test]
async fn simultaneous_runs_append_each_new_observation_once() {
    let (server, days) = moving_catalogue().await;
    let (app, state) = app(&server).await;
    let ada = owner(&app, "Ada").await;
    own(&app, &ada, "OP01-001").await;
    own(&app, &ada, "OP01-010").await;
    days.store(2, Ordering::SeqCst);

    let (first, second) = tokio::join!(refresh(&state, &server), refresh(&state, &server));
    assert_eq!(first.appended + second.appended, 2);
    assert_eq!(points(&app, "OP01-001").await.len(), 2);
    assert_eq!(points(&app, "OP01-010").await.len(), 2);
}

#[tokio::test]
async fn a_catalogue_failure_for_one_code_writes_nothing_and_the_run_goes_on() {
    let (server, days) = moving_catalogue().await;
    let (app, state) = app(&server).await;
    let ada = owner(&app, "Ada").await;
    for card in ["OP01-001", "OP01-010", "OP10-001"] {
        own(&app, &ada, card).await;
    }
    Mock::given(path("/api/sets/card/OP01-010/"))
        .respond_with(ResponseTemplate::new(503))
        .with_priority(1)
        .mount(&server)
        .await;
    days.store(1, Ordering::SeqCst);

    assert_eq!(
        refresh(&state, &server).await,
        Pass {
            codes: 3,
            appended: 2,
            failed: 1
        }
    );
    assert_eq!(points(&app, "OP01-010").await, [(day(20), 2.25)]);
    assert_eq!(points(&app, "OP01-001").await.len(), 2);
    assert_eq!(
        points(&app, "OP10-001").await.len(),
        2,
        "after the failed code"
    );
}

#[tokio::test]
async fn a_card_nobody_owns_any_more_keeps_its_details_image_and_history() {
    let (server, days) = moving_catalogue().await;
    let (app, state) = app(&server).await;
    let ada = owner(&app, "Ada").await;
    own(&app, &ada, "OP01-001").await;
    let entry = list(&app, "").await[0]["id"].as_str().unwrap().to_owned();
    let (status, _) = send(
        &app,
        Method::DELETE,
        &format!("/api/household-users/{ada}/binder/entries/{entry}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    days.store(1, Ordering::SeqCst);
    let asked = card_requests(&server).await;

    assert_eq!(
        refresh(&state, &server).await,
        Pass {
            codes: 0,
            appended: 0,
            failed: 0
        }
    );
    assert_eq!(card_requests(&server).await, asked, "nothing asked for");
    assert_eq!(points(&app, "OP01-001").await, [(day(1), 2.25)]);
    let (status, _) = send(&app, Method::GET, "/api/binder/cards/OP01-001/image", None).await;
    assert_eq!(status, StatusCode::OK);
    let (_, searched) = send(
        &app,
        Method::GET,
        "/api/binder/cards?card_code=OP01-001",
        None,
    )
    .await;
    assert_eq!(searched["cards"][0]["name"], "Invented OP01-001");
}

#[tokio::test]
async fn history_converts_each_point_at_its_own_date_within_the_requested_range() {
    let (server, days) = moving_catalogue().await;
    let (app, state) = app(&server).await;
    store_rates(&state).await;
    let ada = owner(&app, "Ada").await;
    own(&app, &ada, "OP01-010").await;
    days.store(10, Ordering::SeqCst);
    refresh(&state, &server).await;

    let (status, body) = history(&app, "OP01-010", "").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({"card_id": "OP01-010", "prices": [
            {"observed_on": day(20), "usd": 2.25, "eur": 1.88, "dkk": 14.06, "rate_date": day(20)},
            {"observed_on": day(10), "usd": 2.75, "eur": 2.29, "dkk": 17.19, "rate_date": day(20)},
        ]})
    );
    let (_, ranged) = history(
        &app,
        "OP01-010",
        &format!("?from={}&to={}", day(15), day(5)),
    )
    .await;
    assert_eq!(ranged["prices"].as_array().unwrap().len(), 1);
    assert_eq!(ranged["prices"][0]["observed_on"], day(10));
    let (_, empty) = history(&app, "OP01-010", &format!("?from={}", day(2))).await;
    assert_eq!(empty["prices"], json!([]));
}

#[tokio::test]
async fn a_point_without_rates_is_still_listed_with_null_conversions() {
    let (server, _) = moving_catalogue().await;
    let (app, _) = app(&server).await;
    let ada = owner(&app, "Ada").await;
    own(&app, &ada, "OP01-001").await;
    let (_, body) = history(&app, "OP01-001", "").await;
    assert_eq!(
        body["prices"][0],
        json!({"observed_on": day(1), "usd": 2.25, "eur": null, "dkk": null, "rate_date": null})
    );
}

#[tokio::test]
async fn a_bad_range_or_unknown_card_is_an_error() {
    let (server, _) = moving_catalogue().await;
    let (app, _) = app(&server).await;
    let ada = owner(&app, "Ada").await;
    own(&app, &ada, "OP01-001").await;
    for query in [
        "?from=yesterday",
        "?to=2026-13-01",
        "?from=2026-05-02&to=2026-05-01",
    ] {
        assert_eq!(
            history(&app, "OP01-001", query).await.0,
            StatusCode::BAD_REQUEST,
            "{query}"
        );
    }
    assert_eq!(history(&app, "OP01-999", "").await.0, StatusCode::NOT_FOUND);
    assert_eq!(
        history(&app, "bad%2Fid", "").await.0,
        StatusCode::BAD_REQUEST
    );
}
