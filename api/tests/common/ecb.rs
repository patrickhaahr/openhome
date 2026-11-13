//! A synthetic ECB reference-rate server: the daily file at its production path.
#![allow(dead_code)]

use openhome_api::services::{
    card_value::{DailyRates, Rate},
    ecb::Ecb,
    exchange_rates,
    training::CalendarDay,
};
use sqlx::SqlitePool;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};

pub const FILE_PATH: &str = "/stats/eurofxref/eurofxref-daily.xml";

/// One stored rate day, as the daily job would have stored it.
pub async fn store_day(db: &SqlitePool, date: &str, usd: &str, dkk: &str) {
    let rates = DailyRates {
        date: CalendarDay::parse(date).unwrap(),
        usd: Rate::parse(usd).unwrap(),
        dkk: Rate::parse(dkk).unwrap(),
    };
    exchange_rates::store(db, &rates).await.unwrap();
}

/// The file's real shape: one dated Cube holding each currency's rate per euro.
pub fn file(date: &str, usd: &str, dkk: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<gesmes:Envelope xmlns:gesmes="http://www.gesmes.org/xml/2002-08-01" xmlns="http://www.ecb.int/vocabulary/2002-08-01/eurofxref">
  <gesmes:subject>Reference rates</gesmes:subject>
  <gesmes:Sender><gesmes:name>European Central Bank</gesmes:name></gesmes:Sender>
  <Cube>
    <Cube time="{date}">
      <Cube currency="USD" rate="{usd}"/>
      <Cube currency="JPY" rate="171.30"/>
      <Cube currency="DKK" rate="{dkk}"/>
    </Cube>
  </Cube>
</gesmes:Envelope>"#
    )
}

pub async fn serve(server: &MockServer, response: ResponseTemplate) {
    server.reset().await;
    Mock::given(method("GET"))
        .and(path(FILE_PATH))
        .respond_with(response)
        .mount(server)
        .await;
}

/// Another path on the same server, such as a redirect's target.
pub async fn also(server: &MockServer, at: &str, response: ResponseTemplate) {
    Mock::given(method("GET"))
        .and(path(at))
        .respond_with(response)
        .mount(server)
        .await;
}

pub async fn serving(body: String) -> MockServer {
    let server = MockServer::start().await;
    serve(&server, ResponseTemplate::new(200).set_body_string(body)).await;
    server
}

pub fn client(server: &MockServer) -> Ecb {
    Ecb::new(format!("{}{FILE_PATH}", server.uri()).parse().unwrap()).unwrap()
}
