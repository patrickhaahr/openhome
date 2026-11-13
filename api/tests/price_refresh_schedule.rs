mod common;

use std::time::Duration;

use axum::http::StatusCode;
use common::binder::*;
use openhome_api::services::{optcg::Optcg, price_refresh};
use serde_json::json;
use wiremock::MockServer;

async fn card_requests(server: &MockServer) -> usize {
    server
        .received_requests()
        .await
        .unwrap()
        .iter()
        .filter(|request| request.url.path().starts_with("/api/sets/card/"))
        .count()
}

#[tokio::test]
async fn restarts_within_a_day_make_one_round_of_catalogue_requests() {
    let server = catalogue().await;
    let (app, state) = app(&server).await;
    let ada = owner(&app, "Ada").await;
    let (status, body) = add(
        &app,
        json!({"owner_id": ada, "card_id": "OP01-001", "request_id": "r"}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let asked = card_requests(&server).await;
    let catalogue = Optcg::new(server.uri().parse().unwrap()).unwrap();

    let first = tokio::spawn(price_refresh::run_scheduler(
        state.db.clone(),
        catalogue.clone(),
    ));
    while card_requests(&server).await == asked {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    tokio::time::sleep(Duration::from_millis(200)).await;
    first.abort();
    assert_eq!(card_requests(&server).await - asked, 1, "the first start");

    for _ in 0..2 {
        let restart = tokio::spawn(price_refresh::run_scheduler(
            state.db.clone(),
            catalogue.clone(),
        ));
        tokio::time::sleep(Duration::from_millis(300)).await;
        restart.abort();
    }
    assert_eq!(
        card_requests(&server).await - asked,
        1,
        "restarts wait for the next due time"
    );
}
