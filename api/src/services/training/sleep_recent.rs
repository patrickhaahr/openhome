//! Bounded recent Sleep Nights, including explicit gaps on Copenhagen wake days.

use chrono::Utc;
use serde::Serialize;
use sqlx::SqlitePool;

use crate::error::Result;

use super::calendar::{CalendarDay, TIME_ZONE_NAME};
use super::noop_source::NoopSync;
use super::recovery::{self, NoopCoverage, Sleep};

#[derive(Debug, Serialize)]
pub struct RecentSleepNights {
    pub time_zone: &'static str,
    pub nights: Vec<SleepNight>,
}

#[derive(Debug, Serialize)]
pub struct SleepNight {
    pub wake_day: String,
    pub wake_day_start: String,
    pub wake_day_end: String,
    pub sleep: Sleep,
    pub noop: NoopSync<NoopCoverage>,
}

/// Returns today and the preceding `count - 1` Copenhagen wake days, newest first. The route
/// validates the bound of 1 through 14 before calling this function.
pub async fn recent_sleep_nights(pool: &SqlitePool, count: u8) -> Result<RecentSleepNights> {
    let mut day = CalendarDay::of(Utc::now());
    let mut nights = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let recovery = recovery::recovery_day(pool, day).await?;
        nights.push(SleepNight {
            wake_day: recovery.day,
            wake_day_start: recovery.day_start,
            wake_day_end: recovery.day_end,
            sleep: recovery.sleep,
            noop: recovery.noop,
        });
        day = day.previous();
    }
    Ok(RecentSleepNights {
        time_zone: TIME_ZONE_NAME,
        nights,
    })
}
