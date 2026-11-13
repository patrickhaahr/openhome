//! The preview's Price History replay (`OPENHOME_PREVIEW_PRICES=history`): the OPTCG stand-in
//! moves forward one day at a time through the real daily refresh job. Days [`DOWN`] are skipped,
//! as if the service had not run, so every owned Card's history has a gap. On [`FAILING_DAY`] the
//! catalogue refuses one code, so that Card has a one-day gap while the rest of the run goes on.
//! The last pass runs again without moving the catalogue and adds nothing. Every pass is logged to
//! stderr (`Price refresh stand-in: ...`).

use std::time::Duration;

use openhome_api::services::price_refresh;
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
