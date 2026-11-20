use std::time::Duration;

use openhome_api::services::{optcg::Optcg, price_refresh};
use sqlx::SqlitePool;

use super::binder::{Catalogue, HISTORY_DAYS};

const DOWN: [i64; 3] = [5, 6, 7];
const FAILING_DAY: i64 = 9;
const FAILING_CODE: &str = "OP99-013";

pub async fn replay(db: &SqlitePool, catalogue: &Catalogue) -> anyhow::Result<()> {
    for day in 1..=HISTORY_DAYS {
        if DOWN.contains(&day) {
            continue;
        }
        catalogue.control.advance_to(day);
        catalogue
            .control
            .fail((day == FAILING_DAY).then_some(FAILING_CODE));
        let pass = price_refresh::refresh(db, &catalogue.client, Duration::ZERO).await?;
        eprintln!("Price refresh stand-in: day {day}: {pass:?}");
    }
    catalogue.control.fail(None);
    let pass = price_refresh::refresh(db, &catalogue.client, Duration::ZERO).await?;
    eprintln!("Price refresh stand-in: unchanged observations: {pass:?}");
    Ok(())
}

const FIRST_START_SETTLE: Duration = Duration::from_secs(25);
const RESTART_SETTLE: Duration = Duration::from_secs(3);

pub async fn restarts(db: SqlitePool, catalogue: Optcg) {
    for start in 1..=3 {
        eprintln!("Price scheduler stand-in: start {start}");
        let task = tokio::spawn(price_refresh::run_scheduler(db.clone(), catalogue.clone()));
        tokio::time::sleep(if start == 1 {
            FIRST_START_SETTLE
        } else {
            RESTART_SETTLE
        })
        .await;
        task.abort();
    }
    eprintln!("Price scheduler stand-in: done");
}
