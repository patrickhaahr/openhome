use std::time::Duration;

use axum::{Router, http::StatusCode, response::IntoResponse, routing::get};
use chrono::{Datelike, NaiveDate, Weekday};
use openhome_api::services::{ecb::Ecb, exchange_rates, training::CalendarDay};
use sqlx::SqlitePool;

const HISTORY_DAYS: u64 = 365;
const RECENT_DAYS: u64 = 90;
const MISSED_DAYS_AGO: u64 = 31;
pub const NEXT_PASS: Duration = Duration::from_secs(90);
const RECENT_PATH: &str = "/stats/eurofxref/eurofxref-hist-90d.xml";
const HISTORY_PATH: &str = "/stats/eurofxref/eurofxref-hist.xml";

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Outage,
    MissedDay,
}

pub fn mode() -> Mode {
    match std::env::var("OPENHOME_PREVIEW_ECB").as_deref() {
        Ok("outage") => Mode::Outage,
        Ok("missed-day") => Mode::MissedDay,
        _ => Mode::Normal,
    }
}

fn date(day: CalendarDay) -> NaiveDate {
    NaiveDate::parse_from_str(&day.to_string(), "%Y-%m-%d").expect("a calendar day is a date")
}

fn working_day(mut day: CalendarDay) -> CalendarDay {
    while matches!(date(day).weekday(), Weekday::Sat | Weekday::Sun) {
        day = day.previous();
    }
    day
}

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

fn working_days(span: u64) -> Vec<CalendarDay> {
    let newest = latest();
    (0..span)
        .filter_map(|age| newest.checked_sub_days(age))
        .filter(|day| working_day(*day) == *day)
        .collect()
}

fn recent_file() -> String {
    envelope(
        &working_days(RECENT_DAYS)
            .into_iter()
            .map(cube)
            .collect::<String>(),
    )
}

fn history_file(mode: Mode) -> String {
    let missed = (mode == Mode::MissedDay)
        .then(|| CalendarDay::of(chrono::Utc::now()).checked_sub_days(MISSED_DAYS_AGO))
        .flatten()
        .map(working_day);
    let days: Vec<_> = working_days(HISTORY_DAYS)
        .into_iter()
        .filter(|day| Some(*day) != missed)
        .collect();
    eprintln!(
        "ECB stand-in: history holds {} working days, {} to {}{}",
        days.len(),
        days.last().map_or_else(String::new, ToString::to_string),
        latest(),
        missed.map_or_else(String::new, |day| format!(", without {day}"))
    );
    envelope(&days.into_iter().map(cube).collect::<String>())
}

fn answer(down: bool, path: &str, body: impl FnOnce() -> String) -> impl IntoResponse {
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

pub async fn ecb(mode: Mode) -> anyhow::Result<Ecb> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let down = mode == Mode::Outage;
    let provider = Router::new()
        .route(
            RECENT_PATH,
            get(move || async move { answer(down, RECENT_PATH, recent_file) }),
        )
        .route(
            HISTORY_PATH,
            get(move || async move { answer(down, HISTORY_PATH, move || history_file(mode)) }),
        );
    tokio::spawn(async move { axum::serve(listener, provider).await });
    eprintln!(
        "ECB: loopback reference rates at {address}{}",
        if down { " (outage: answers 503)" } else { "" }
    );
    let at = |path: &str| format!("http://{address}{path}").parse();
    Ecb::new(at(RECENT_PATH)?, at(HISTORY_PATH)?)
}

pub async fn next_pass(db: SqlitePool, ecb: Ecb) {
    tokio::time::sleep(NEXT_PASS).await;
    eprintln!("ECB stand-in: the next day's pass");
    match exchange_rates::refresh(&db, &ecb).await {
        Ok(pass) => eprintln!("ECB stand-in: the next day's pass stored {pass:?}"),
        Err(error) => eprintln!("ECB stand-in: the next day's pass failed: {error}"),
    }
}
