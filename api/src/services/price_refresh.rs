use std::sync::LazyLock;
use std::time::Duration;

use sqlx::SqlitePool;
use tokio::sync::Mutex;

use super::cards::CardCode;
use super::optcg::Optcg;
use super::price_history;

static RUN: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

const PACE: Duration = Duration::from_secs(2);
const REFRESH_INTERVAL: Duration = Duration::from_hours(24);
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

async fn due_in(db: &SqlitePool) -> Result<Option<Duration>, sqlx::Error> {
    let due = sqlx::query_scalar!("SELECT next_due_at FROM price_refresh_state WHERE id = 1")
        .fetch_optional(db)
        .await?;
    let wait = due.map_or(0, |due| due - chrono::Utc::now().timestamp());
    Ok((wait > 0).then(|| Duration::from_secs(wait.unsigned_abs()).min(REFRESH_INTERVAL)))
}

async fn schedule_next(db: &SqlitePool, after: Duration) -> Result<(), sqlx::Error> {
    let due = chrono::Utc::now().timestamp() + after.as_secs().cast_signed();
    sqlx::query!(
        "INSERT INTO price_refresh_state (id, next_due_at) VALUES (1, ?)
         ON CONFLICT (id) DO UPDATE SET next_due_at = excluded.next_due_at",
        due
    )
    .execute(db)
    .await?;
    Ok(())
}

async fn run_pass(db: &SqlitePool, catalogue: &Optcg) -> Result<(), sqlx::Error> {
    schedule_next(db, REFRESH_INTERVAL).await?;
    match refresh(db, catalogue, PACE).await {
        Ok(Pass {
            codes,
            appended,
            failed,
        }) => {
            tracing::info!(codes, appended, failed, "Card Price refresh finished");
            Ok(())
        }
        Err(error) => {
            tracing::warn!(%error, "Card Price refresh failed");
            schedule_next(db, RETRY_PAUSE).await
        }
    }
}

pub async fn run_scheduler(db: SqlitePool, catalogue: Optcg) {
    tracing::info!("Starting Card Price refresh (daily, persisted across restarts)");
    loop {
        let wait = match due_in(&db).await {
            Ok(Some(wait)) => {
                tracing::info!(
                    wait_secs = wait.as_secs(),
                    "Card Price refresh is not due yet"
                );
                wait
            }
            Ok(None) => match run_pass(&db, &catalogue).await {
                Ok(()) => continue,
                Err(error) => {
                    tracing::warn!(%error, "Card Price refresh could not schedule its next pass");
                    RETRY_PAUSE
                }
            },
            Err(error) => {
                tracing::warn!(%error, "Card Price refresh could not read its due time");
                RETRY_PAUSE
            }
        };
        tokio::time::sleep(wait).await;
    }
}
