//! The preview's Price History replay (`OPENHOME_PREVIEW_PRICES=history`): the OPTCG stand-in
//! moves forward one day at a time through the real daily refresh job. Days [`DOWN`] are skipped,
//! as if the service had not run, so every owned Card's history has a gap. On [`FAILING_DAY`] the
//! catalogue refuses one code, so that Card has a one-day gap while the rest of the run goes on.
//! The last pass runs again without moving the catalogue and adds nothing. Every pass is logged to
//! stderr (`Price refresh stand-in: ...`). The replay calls the refresh directly, so it never
//! touches the scheduler's persisted due time. After it, the real scheduler is started three times
//! on the same database, as a crash loop would: only the first start asks the catalogue
//! (`Price scheduler stand-in: start N` marks each start in the stack log).

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

/// One pass over the nine owned codes at the scheduler's two-second pace takes about eighteen
/// seconds; a start that skips its pass needs only a moment to say so.
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
