//! Stored ECB rates and the daily job that adds to them. A pass that cannot read the ECB writes
//! nothing, so the previous rates stay and values keep converting at the latest earlier day.
//! A price is converted at the latest stored day at or before its own, never a later one, so the
//! first pass on a store that does not reach back to the oldest stored price backfills the
//! ECB's full history, and every other pass stores the 90-day file, so a day the service
//! missed is filled by the next pass.

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
    Recent { days: usize },
    History { days: usize },
}

/// The 90-day file holds the last ~64 working days. A stored newest day older than this leaves a
/// gap it may not reach, so the margin keeps its first day certain to be at or before the newest
/// stored one.
const RECENT_REACH_DAYS: u64 = 80;

/// The stored rates cannot be completed from the 90-day file: they are empty, start after the
/// day the oldest stored Card Price was observed, or end before the file's first day.
async fn needs_history(db: &SqlitePool) -> Result<bool, sqlx::Error> {
    let row = sqlx::query!(
        r#"SELECT (SELECT MIN(rate_date) FROM exchange_rates) AS "earliest_rate?: String",
                  (SELECT MAX(rate_date) FROM exchange_rates) AS "latest_rate?: String",
                  (SELECT MIN(price_observed_on) FROM cards) AS "oldest_price?: String""#
    )
    .fetch_one(db)
    .await?;
    let Some((earliest, latest)) = row.earliest_rate.zip(row.latest_rate) else {
        return Ok(true);
    };
    let reach = CalendarDay::today()
        .checked_sub_days(RECENT_REACH_DAYS)
        .map_or_else(String::new, |day| day.to_string());
    Ok(latest < reach || row.oldest_price.is_some_and(|price| earliest > price))
}

/// One pass: read the ECB's full history when the stored rates cannot be completed from its
/// 90-day file, else the 90-day file, and store every day it holds. Reads first and writes once,
/// so every failure leaves the stored rates untouched.
pub async fn refresh(db: &SqlitePool, ecb: &Ecb) -> Result<Pass, RefreshError> {
    let history = needs_history(db).await?;
    let days = if history {
        ecb.history().await
    } else {
        ecb.recent().await
    }
    .map_err(RefreshError::Unavailable)?;
    store(db, &days).await?;
    let days = days.len();
    Ok(if history {
        Pass::History { days }
    } else {
        Pass::Recent { days }
    })
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
            Ok(Pass::Recent { days }) => {
                tracing::info!(days, "Stored ECB reference rates of the last 90 days");
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
