//! Stored ECB rates and the daily job that adds to them. A pass that cannot read the ECB writes
//! nothing, so the previous rates stay and values keep converting at the latest earlier day.
//! Prices older than the first stored day have no rates to convert at and read as unpriced.

use std::time::Duration;

use sqlx::SqlitePool;

use super::card_value::{DailyRates, Rate, Rates, Valuer};
use super::ecb::{Ecb, Unavailable};
use super::training::CalendarDay;

#[derive(Debug, thiserror::Error)]
pub enum RefreshError {
    #[error("the ECB reference rates are unavailable")]
    Unavailable(Unavailable),
    #[error("exchange rate database operation failed")]
    Database(#[from] sqlx::Error),
}

/// Every stored day's rates, valued as of today in Copenhagen.
pub async fn valuer(db: &SqlitePool) -> Result<Valuer, sqlx::Error> {
    Ok(Valuer::new(load(db).await?, CalendarDay::today()))
}

pub async fn load(db: &SqlitePool) -> Result<Rates, sqlx::Error> {
    let rows = sqlx::query!("SELECT rate_date, usd_micros, dkk_micros FROM exchange_rates")
        .fetch_all(db)
        .await?;
    let decode = |what: &str| sqlx::Error::Decode(format!("stored {what} is invalid").into());
    rows.into_iter()
        .map(|row| {
            Ok(DailyRates {
                date: CalendarDay::parse(&row.rate_date).ok_or_else(|| decode("rate date"))?,
                usd: Rate::from_micros(row.usd_micros).ok_or_else(|| decode("USD rate"))?,
                dkk: Rate::from_micros(row.dkk_micros).ok_or_else(|| decode("DKK rate"))?,
            })
        })
        .collect::<Result<Vec<_>, sqlx::Error>>()
        .map(Rates::new)
}

/// The ECB publishes a day once, so a repeated read of the same day converges on the same row.
pub async fn store(db: &SqlitePool, rates: &DailyRates) -> Result<(), sqlx::Error> {
    let date = rates.date.to_string();
    let (usd, dkk) = (rates.usd.micros(), rates.dkk.micros());
    sqlx::query!(
        "INSERT INTO exchange_rates (rate_date, usd_micros, dkk_micros) VALUES (?, ?, ?)
         ON CONFLICT(rate_date) DO UPDATE
         SET usd_micros = excluded.usd_micros, dkk_micros = excluded.dkk_micros",
        date,
        usd,
        dkk
    )
    .execute(db)
    .await?;
    Ok(())
}

/// One pass: read the ECB's current file and store its day. Reads first and writes once, so
/// every failure leaves the stored rates untouched.
pub async fn refresh(db: &SqlitePool, ecb: &Ecb) -> Result<DailyRates, RefreshError> {
    let rates = ecb.daily().await.map_err(RefreshError::Unavailable)?;
    store(db, &rates).await?;
    Ok(rates)
}

const REFRESH_INTERVAL: Duration = Duration::from_hours(24);

/// The daily schedule, run for the life of the process on a detached task: a pass at startup,
/// then one every 24 hours after the previous pass ends. A failed pass is logged and waits for
/// the next one. There is no shutdown signal: the task ends only with the runtime.
pub async fn run_scheduler(db: SqlitePool, ecb: Ecb) {
    tracing::info!("Starting ECB rate refresh (initial fetch + 24h interval)");
    loop {
        match refresh(&db, &ecb).await {
            Ok(rates) => tracing::info!(date = %rates.date, "Stored ECB reference rates"),
            Err(error) => {
                tracing::warn!(%error, "ECB rate refresh failed; keeping the previous rates");
            }
        }
        tokio::time::sleep(REFRESH_INTERVAL).await;
    }
}
