//! The preview's ECB stand-in: the daily reference-rate file at its production path, in the real
//! file's shape, with invented rates that drift a little each day. It serves the latest working
//! day on or before today in Copenhagen, and every request is logged to stderr
//! (`ECB stand-in: GET <path> -> <status>`). `OPENHOME_PREVIEW_ECB=outage` makes it answer 503, so the startup
//! refresh fails and the earlier rates stay. [`seed_history`] stores the working days before
//! today, so prices observed months ago still convert and a price observed today shows whether
//! today's rates arrived.

use axum::{Router, extract::State, http::StatusCode, response::IntoResponse, routing::get};
use chrono::{Datelike, NaiveDate, Weekday};
use openhome_api::services::{
    card_value::{DailyRates, Rate},
    ecb::Ecb,
    exchange_rates,
    training::CalendarDay,
};
use sqlx::SqlitePool;

const HISTORY_DAYS: u64 = 200;
const FILE_PATH: &str = "/stats/eurofxref/eurofxref-daily.xml";

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

fn file(day: CalendarDay) -> String {
    let (usd, dkk) = rates(day);
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<gesmes:Envelope xmlns:gesmes="http://www.gesmes.org/xml/2002-08-01" xmlns="http://www.ecb.int/vocabulary/2002-08-01/eurofxref">
	<gesmes:subject>Reference rates</gesmes:subject>
	<gesmes:Sender><gesmes:name>European Central Bank</gesmes:name></gesmes:Sender>
	<Cube>
		<Cube time="{day}">
			<Cube currency="USD" rate="{usd}"/>
			<Cube currency="JPY" rate="171.30"/>
			<Cube currency="DKK" rate="{dkk}"/>
		</Cube>
	</Cube>
</gesmes:Envelope>"#
    )
}

async fn daily(State(down): State<bool>) -> impl IntoResponse {
    let (status, body) = if down {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            "synthetic outage".to_owned(),
        )
    } else {
        (
            StatusCode::OK,
            file(working_day(CalendarDay::of(chrono::Utc::now()))),
        )
    };
    eprintln!("ECB stand-in: GET {FILE_PATH} -> {}", status.as_u16());
    (status, body)
}

/// The rate client for a loopback stand-in. Never the live ECB.
pub async fn ecb() -> anyhow::Result<Ecb> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let down = outage();
    let provider = Router::new().route(FILE_PATH, get(daily)).with_state(down);
    tokio::spawn(async move { axum::serve(listener, provider).await });
    // Without the scheme: test harnesses take the first `http://127.0.0.1:<port>` they see as Axum's URL.
    eprintln!(
        "ECB: loopback reference rates at {address}{}",
        if down { " (outage: answers 503)" } else { "" }
    );
    Ecb::new(format!("http://{address}{FILE_PATH}").parse()?)
}

/// The working days before today, stored as the daily job would have stored them.
pub async fn seed_history(db: &SqlitePool) -> anyhow::Result<()> {
    let today = CalendarDay::of(chrono::Utc::now());
    for age in 1..=HISTORY_DAYS {
        let day = today
            .checked_sub_days(age)
            .ok_or_else(|| anyhow::anyhow!("history reaches before the calendar"))?;
        if working_day(day) != day {
            continue;
        }
        let (usd, dkk) = rates(day);
        let parse = |text: &str| Rate::parse(text).ok_or_else(|| anyhow::anyhow!("rate {text}"));
        exchange_rates::store(
            db,
            &DailyRates {
                date: day,
                usd: parse(&usd)?,
                dkk: parse(&dkk)?,
            },
        )
        .await?;
    }
    Ok(())
}
