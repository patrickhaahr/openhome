//! Stored ECB rates and the daily job that adds to them. A pass that cannot read the ECB writes
//! nothing, so the previous rates stay and values keep converting at the latest earlier day.
//! A price is converted at the latest stored day at or before its own, never a later one, so the
//! first pass on a store that does not reach back to the oldest stored price backfills the
//! ECB's full history instead of the current day alone.

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
/// All days go in one transaction: a failure keeps the previous rates.
pub async fn store(db: &SqlitePool, days: &[DailyRates]) -> Result<(), sqlx::Error> {
    let mut tx = db.begin().await?;
    for rates in days {
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
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await
}

/// What a pass stored.
#[derive(Debug)]
pub enum Pass {
    Daily(DailyRates),
    History { days: usize },
}

/// The stored rates leave the oldest stored Card Price unconvertible: they are empty, or start
/// after the day it was observed.
async fn needs_history(db: &SqlitePool) -> Result<bool, sqlx::Error> {
    let row = sqlx::query!(
        r#"SELECT (SELECT MIN(rate_date) FROM exchange_rates) AS "earliest_rate?: String",
                  (SELECT MIN(price_observed_on) FROM cards) AS "oldest_price?: String""#
    )
    .fetch_one(db)
    .await?;
    Ok(match (row.earliest_rate, row.oldest_price) {
        (None, _) => true,
        (Some(_), None) => false,
        (Some(rate), Some(price)) => rate > price,
    })
}

/// One pass: read the ECB's full history when the stored rates do not cover the oldest stored
/// Card Price, else its current file, and store what it holds. Reads first and writes once, so
/// every failure leaves the stored rates untouched.
pub async fn refresh(db: &SqlitePool, ecb: &Ecb) -> Result<Pass, RefreshError> {
    if needs_history(db).await? {
        let days = ecb.history().await.map_err(RefreshError::Unavailable)?;
        store(db, &days).await?;
        return Ok(Pass::History { days: days.len() });
    }
    let rates = ecb.daily().await.map_err(RefreshError::Unavailable)?;
    store(db, &[rates]).await?;
    Ok(Pass::Daily(rates))
}

const REFRESH_INTERVAL: Duration = Duration::from_hours(24);
/// A failed pass is retried sooner than a day, so a startup before the network is up does not
/// leave the household without rates until tomorrow.
const RETRY_PAUSE: Duration = Duration::from_mins(15);

/// The daily schedule, run for the life of the process on a detached task: a pass at startup,
/// then one every 24 hours after the previous pass ends, or after [`RETRY_PAUSE`] when it
/// failed. A failed pass is logged. There is no shutdown signal: the task ends only with the
/// runtime.
pub async fn run_scheduler(db: SqlitePool, ecb: Ecb) {
    tracing::info!("Starting ECB rate refresh (initial fetch + 24h interval)");
    loop {
        let pause = match refresh(&db, &ecb).await {
            Ok(Pass::Daily(rates)) => {
                tracing::info!(date = %rates.date, "Stored ECB reference rates");
                REFRESH_INTERVAL
            }
            Ok(Pass::History { days }) => {
                tracing::info!(days, "Stored ECB reference rate history");
                REFRESH_INTERVAL
            }
            Err(error) => {
                tracing::warn!(%error, "ECB rate refresh failed; keeping the previous rates");
                RETRY_PAUSE
            }
        };
        tokio::time::sleep(pause).await;
    }
}
