//! The preview's ECB stand-in: the daily and full-history reference-rate files at their
//! production paths, in the real files' shape, with invented rates that drift a little each day.
//! The daily file holds the latest working day on or before today in Copenhagen. The history
//! file holds every working day of the last year up to that one, newest first on a single line,
//! so the startup pass backfills prices observed months ago. Every request is logged to stderr
//! (`ECB stand-in: GET <path> -> <status>`). `OPENHOME_PREVIEW_ECB=outage` makes both answer
//! 503, so the startup pass fails and no rates are stored.

use axum::{Router, http::StatusCode, response::IntoResponse, routing::get};
use chrono::{Datelike, NaiveDate, Weekday};
use openhome_api::services::{ecb::Ecb, training::CalendarDay};

const HISTORY_DAYS: u64 = 365;
const DAILY_PATH: &str = "/stats/eurofxref/eurofxref-daily.xml";
const HISTORY_PATH: &str = "/stats/eurofxref/eurofxref-hist.xml";

fn outage() -> bool {
    std::env::var("OPENHOME_PREVIEW_ECB").is_ok_and(|mode| mode == "outage")
}

fn date(day: CalendarDay) -> NaiveDate {
    NaiveDate::parse_from_str(&day.to_string(), "%Y-%m-%d").expect("a calendar day is a date")
}

/// The ECB publishes on working days only.
fn working_day(mut day: CalendarDay) -> CalendarDay {
    while matches!(date(day).weekday(), Weekday::Sat | Weekday::Sun) {
        day = day.previous();
    }
    day
}

/// Invented USD and DKK per euro with four decimals, stable for a given day.
fn rates(day: CalendarDay) -> (String, String) {
    let n = u64::try_from(date(day).num_days_from_ce()).expect("dates are after year zero");
    let (usd, dkk) = (10_800 + n * 7 % 600, 74_600 + n * 3 % 40);
    let text = |units: u64| format!("{}.{:04}", units / 10_000, units % 10_000);
    (text(usd), text(dkk))
}

fn cube(day: CalendarDay) -> String {
    let (usd, dkk) = rates(day);
    format!(
        r#"<Cube time="{day}"><Cube currency="USD" rate="{usd}"/><Cube currency="JPY" rate="171.30"/><Cube currency="DKK" rate="{dkk}"/></Cube>"#
    )
}

fn envelope(cubes: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<gesmes:Envelope xmlns:gesmes="http://www.gesmes.org/xml/2002-08-01" xmlns="http://www.ecb.int/vocabulary/2002-08-01/eurofxref"><gesmes:subject>Reference rates</gesmes:subject><gesmes:Sender><gesmes:name>European Central Bank</gesmes:name></gesmes:Sender><Cube>{cubes}</Cube></gesmes:Envelope>"#
    )
}

fn latest() -> CalendarDay {
    working_day(CalendarDay::of(chrono::Utc::now()))
}

fn daily_file() -> String {
    envelope(&cube(latest()))
}

fn history_file() -> String {
    let newest = latest();
    let days: Vec<_> = (0..HISTORY_DAYS)
        .filter_map(|age| newest.checked_sub_days(age))
        .filter(|day| working_day(*day) == *day)
        .collect();
    eprintln!(
        "ECB stand-in: history holds {} working days, {} to {newest}",
        days.len(),
        days.last().map_or_else(String::new, ToString::to_string)
    );
    envelope(&days.into_iter().map(cube).collect::<String>())
}

fn answer(down: bool, path: &str, body: fn() -> String) -> impl IntoResponse {
    let (status, body) = if down {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            "synthetic outage".to_owned(),
        )
    } else {
        (StatusCode::OK, body())
    };
    eprintln!("ECB stand-in: GET {path} -> {}", status.as_u16());
    (status, body)
}

/// The rate client for a loopback stand-in. Never the live ECB.
pub async fn ecb() -> anyhow::Result<Ecb> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let down = outage();
    let provider = Router::new()
        .route(
            DAILY_PATH,
            get(move || async move { answer(down, DAILY_PATH, daily_file) }),
        )
        .route(
            HISTORY_PATH,
            get(move || async move { answer(down, HISTORY_PATH, history_file) }),
        );
    tokio::spawn(async move { axum::serve(listener, provider).await });
    // Without the scheme: test harnesses take the first `http://127.0.0.1:<port>` they see as Axum's URL.
    eprintln!(
        "ECB: loopback reference rates at {address}{}",
        if down { " (outage: answers 503)" } else { "" }
    );
    let at = |path: &str| format!("http://{address}{path}").parse();
    Ecb::new(at(DAILY_PATH)?, at(HISTORY_PATH)?)
}
