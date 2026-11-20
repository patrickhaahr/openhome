#![allow(dead_code)]

use std::fmt::Write;

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

pub const RECENT_PATH: &str = "/stats/eurofxref/eurofxref-hist-90d.xml";
pub const HISTORY_PATH: &str = "/stats/eurofxref/eurofxref-hist.xml";

pub async fn store_day(db: &SqlitePool, date: &str, usd: &str, dkk: &str) {
    let rates = DailyRates {
        date: CalendarDay::parse(date).unwrap(),
        usd: Rate::parse(usd).unwrap(),
        dkk: Rate::parse(dkk).unwrap(),
    };
    exchange_rates::store(db, &[rates]).await.unwrap();
}

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

pub fn history_file(days: &[(&str, &str, Option<&str>)]) -> String {
    let mut cubes = String::new();
    for (date, usd, dkk) in days {
        let dkk = dkk.map_or_else(String::new, |rate| {
            format!(r#"<Cube currency="DKK" rate="{rate}"/>"#)
        });
        write!(
            cubes,
            r#"<Cube time="{date}"><Cube currency="USD" rate="{usd}"/><Cube currency="JPY" rate="171.30"/>{dkk}</Cube>"#
        )
        .unwrap();
    }
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<gesmes:Envelope xmlns:gesmes="http://www.gesmes.org/xml/2002-08-01" xmlns="http://www.ecb.int/vocabulary/2002-08-01/eurofxref"><gesmes:subject>Reference rates</gesmes:subject><gesmes:Sender><gesmes:name>European Central Bank</gesmes:name></gesmes:Sender><Cube>{cubes}</Cube></gesmes:Envelope>"#
    )
}

pub async fn serve(server: &MockServer, response: ResponseTemplate) {
    server.reset().await;
    Mock::given(method("GET"))
        .and(path(RECENT_PATH))
        .respond_with(response)
        .mount(server)
        .await;
}

pub async fn serve_history(server: &MockServer, response: ResponseTemplate) {
    also(server, HISTORY_PATH, response).await;
}

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
    let at = |path: &str| format!("{}{path}", server.uri()).parse().unwrap();
    Ecb::new(at(RECENT_PATH), at(HISTORY_PATH)).unwrap()
}
