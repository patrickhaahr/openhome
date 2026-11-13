//! The daily Card Price refresh: one paced OPTCG request per distinct Card Code that has a Binder
//! Entry, appending a Price Point for each owned variant whose observation date is newer than its
//! latest. Unowned Cards are never asked for or touched, so they keep their details, image and
//! history. A code the catalogue fails on is logged by its code alone, never upstream text, and
//! the run goes on; only a database failure ends a pass.

use std::sync::LazyLock;
use std::time::Duration;

use sqlx::SqlitePool;
use tokio::sync::Mutex;

use super::cards::CardCode;
use super::optcg::Optcg;
use super::price_history;

/// One run at a time, so the pacing between requests holds across overlapping runs.
static RUN: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

/// The catalogue is one volunteer's server: a pause between codes keeps a run gentle.
const PACE: Duration = Duration::from_secs(2);
const REFRESH_INTERVAL: Duration = Duration::from_hours(24);
/// A pass the database failed is retried sooner than a day.
const RETRY_PAUSE: Duration = Duration::from_mins(15);

#[derive(Debug, PartialEq, Eq)]
pub struct Pass {
    pub codes: usize,
    pub appended: usize,
    pub failed: usize,
}

async fn owned_codes(db: &SqlitePool) -> Result<Vec<CardCode>, sqlx::Error> {
    sqlx::query_scalar!(
        r#"SELECT DISTINCT cards.card_code AS "card_code!" FROM cards
           JOIN binder_entries ON binder_entries.card_id = cards.id ORDER BY cards.card_code"#
    )
    .fetch_all(db)
    .await?
    .iter()
    .map(|code| {
        CardCode::parse(code).map_err(|_| sqlx::Error::Decode("stored Card Code is invalid".into()))
    })
    .collect()
}

pub async fn refresh(
    db: &SqlitePool,
    catalogue: &Optcg,
    pace: Duration,
) -> Result<Pass, sqlx::Error> {
    let _run = RUN.lock().await;
    let codes = owned_codes(db).await?;
    let mut pass = Pass {
        codes: codes.len(),
        appended: 0,
        failed: 0,
    };
    for (index, code) in codes.iter().enumerate() {
        if index > 0 {
            tokio::time::sleep(pace).await;
        }
        let Ok(variants) = catalogue.card(code).await else {
            tracing::warn!(
                card_code = code.as_str(),
                "Card Price refresh: the catalogue failed for this code"
            );
            pass.failed += 1;
            continue;
        };
        for variant in variants {
            let Some(price) = variant.price else { continue };
            if price_history::record_newer(db, variant.id.as_str(), price.usd, &price.observed_on)
                .await?
            {
                pass.appended += 1;
            }
        }
    }
    Ok(pass)
}

/// The daily schedule, run for the life of the process on a detached task: a pass at startup,
/// then one 24 hours after each pass ends, or after [`RETRY_PAUSE`] when the database failed one.
pub async fn run_scheduler(db: SqlitePool, catalogue: Optcg) {
    tracing::info!("Starting Card Price refresh (initial pass + 24h interval)");
    loop {
        let pause = match refresh(&db, &catalogue, PACE).await {
            Ok(Pass {
                codes,
                appended,
                failed,
            }) => {
                tracing::info!(codes, appended, failed, "Card Price refresh finished");
                REFRESH_INTERVAL
            }
            Err(error) => {
                tracing::warn!(%error, "Card Price refresh failed");
                RETRY_PAUSE
            }
        };
        tokio::time::sleep(pause).await;
    }
}
