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

#[derive(Debug)]
pub enum Pass {
    Recent { days: usize },
    History { days: usize },
}

const RECENT_REACH_DAYS: u64 = 80;

async fn needs_history(db: &SqlitePool) -> Result<bool, sqlx::Error> {
    let row = sqlx::query!(
        r#"SELECT (SELECT MIN(rate_date) FROM exchange_rates) AS "earliest_rate?: String",
                  (SELECT MAX(rate_date) FROM exchange_rates) AS "latest_rate?: String",
                  (SELECT MIN(observed_on) FROM price_points) AS "oldest_price?: String""#
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
const RETRY_PAUSE: Duration = Duration::from_mins(15);

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
