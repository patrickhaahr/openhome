//! The preview's ECB stand-in: the 90-day and full-history reference-rate files at their
//! production paths, in the real files' shape, with invented rates that drift a little each day.
//! The 90-day file holds every working day of the last 90 calendar days up to the latest working
//! day on or before today in Copenhagen. The history file holds every working day of the last
//! year, newest first on a single line, so the startup pass backfills prices observed months ago.
//! Every request is logged to stderr (`ECB stand-in: GET <path> -> <status>`).
//! `OPENHOME_PREVIEW_ECB=outage` makes both answer 503, so the startup pass fails and no rates
//! are stored. `OPENHOME_PREVIEW_ECB=missed-day` leaves one working day out of the history file,
//! as if the service had been down that day, and runs a second pass [`NEXT_PASS`] after startup
//! as the next day's pass would, whose 90-day file holds the day.

use std::time::Duration;

use axum::{Router, http::StatusCode, response::IntoResponse, routing::get};
use chrono::{Datelike, NaiveDate, Weekday};
use openhome_api::services::{ecb::Ecb, exchange_rates, training::CalendarDay};
use sqlx::SqlitePool;

const HISTORY_DAYS: u64 = 365;
const RECENT_DAYS: u64 = 90;
/// The seeded `OP99-015` price is this many days old, so its day is the one missed.
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

/// Working days from the latest back `span` calendar days, newest first.
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

/// The rate client for a loopback stand-in. Never the live ECB.
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
    // Without the scheme: test harnesses take the first `http://127.0.0.1:<port>` they see as Axum's URL.
    eprintln!(
        "ECB: loopback reference rates at {address}{}",
        if down { " (outage: answers 503)" } else { "" }
    );
    let at = |path: &str| format!("http://{address}{path}").parse();
    Ecb::new(at(RECENT_PATH)?, at(HISTORY_PATH)?)
}

/// The pass the next day would run, so a missed day's refill is observable in one sitting.
pub async fn next_pass(db: SqlitePool, ecb: Ecb) {
    tokio::time::sleep(NEXT_PASS).await;
    eprintln!("ECB stand-in: the next day's pass");
    match exchange_rates::refresh(&db, &ecb).await {
        Ok(pass) => eprintln!("ECB stand-in: the next day's pass stored {pass:?}"),
        Err(error) => eprintln!("ECB stand-in: the next day's pass failed: {error}"),
    }
}
