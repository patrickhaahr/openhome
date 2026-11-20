use chrono::{DateTime, SecondsFormat, Utc};
use serde::Serialize;
use sqlx::SqlitePool;

use super::calendar::{self, CalendarDay};

pub const IMPORTED_DEVICE_ID: &str = "my-whoop";
pub const COMPUTED_DEVICE_ID: &str = "my-whoop-noop";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stream {
    DailyMetric,
    SleepSession,
    Workout,
    Journal,
}

impl Stream {
    const fn name(self) -> &'static str {
        match self {
            Self::DailyMetric => "dailyMetric",
            Self::SleepSession => "sleepSession",
            Self::Workout => "workout",
            Self::Journal => "journal",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Rows {
    DailyMetrics(CalendarDay),
    SleepSessions(CalendarDay),
    Workouts(CalendarDay),
    Journal(CalendarDay),
}

#[derive(Debug, Clone)]
pub struct Installation {
    receiver_state_id: String,
    pub source_id: String,
}

pub async fn active_installation(pool: &SqlitePool) -> anyhow::Result<Option<Installation>> {
    let row: Option<(String, String)> = sqlx::query_as(
        "SELECT l.receiver_state_id, l.source_id FROM batch_ledger l \
         JOIN receiver_state r ON r.id = 1 AND r.receiver_state_id = l.receiver_state_id \
         WHERE l.device_id IN (?, ?) \
         ORDER BY l.accepted_at DESC LIMIT 1",
    )
    .bind(IMPORTED_DEVICE_ID)
    .bind(COMPUTED_DEVICE_ID)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|(receiver_state_id, source_id)| Installation {
        receiver_state_id,
        source_id,
    }))
}

#[derive(Debug, Clone, Copy, sqlx::FromRow)]
pub struct HrSample {
    pub ts: i64,
    pub bpm: i64,
}

pub async fn strap_hr_samples(
    pool: &SqlitePool,
    installation: &Installation,
    from: i64,
    to: i64,
) -> anyhow::Result<Vec<HrSample>> {
    Ok(sqlx::query_as(
        "SELECT ts, bpm FROM hr_sample \
         WHERE source_id = ? AND device_id = ? AND ts >= ? AND ts <= ? ORDER BY ts",
    )
    .bind(&installation.source_id)
    .bind(IMPORTED_DEVICE_ID)
    .bind(from)
    .bind(to)
    .fetch_all(pool)
    .await?)
}

async fn stream_devices(
    pool: &SqlitePool,
    installation: &Installation,
    stream: Stream,
) -> anyhow::Result<Vec<String>> {
    match stream {
        Stream::DailyMetric | Stream::SleepSession => Ok(vec![
            IMPORTED_DEVICE_ID.to_owned(),
            COMPUTED_DEVICE_ID.to_owned(),
        ]),
        Stream::Workout => Ok(sqlx::query_scalar(
            "SELECT ? AS device_id UNION SELECT ? \
             UNION SELECT device_id FROM workout WHERE source_id = ? \
             UNION SELECT device_id FROM replacement \
             WHERE receiver_state_id = ? AND source_id = ? AND stream = 'workout'",
        )
        .bind(IMPORTED_DEVICE_ID)
        .bind(COMPUTED_DEVICE_ID)
        .bind(&installation.source_id)
        .bind(&installation.receiver_state_id)
        .bind(&installation.source_id)
        .fetch_all(pool)
        .await?),
        Stream::Journal => Ok(sqlx::query_scalar(
            "SELECT device_id FROM journal WHERE source_id = ? \
             UNION SELECT device_id FROM replacement \
             WHERE receiver_state_id = ? AND source_id = ? AND stream = 'journal'",
        )
        .bind(&installation.source_id)
        .bind(&installation.receiver_state_id)
        .bind(&installation.source_id)
        .fetch_all(pool)
        .await?),
    }
}

pub async fn last_push_at(
    pool: &SqlitePool,
    installation: &Installation,
    streams: &[Stream],
) -> anyhow::Result<Option<DateTime<Utc>>> {
    let mut oldest: Option<DateTime<Utc>> = None;
    for stream in streams {
        let devices = stream_devices(pool, installation, *stream).await?;
        let latest: Vec<String> = sqlx::query_scalar(
            "SELECT MAX(l.accepted_at) FROM replacement r \
             JOIN replacement_part p USING (receiver_state_id, source_id, device_id, replacement_id) \
             JOIN batch_ledger l USING (receiver_state_id, source_id, device_id, batch_id) \
             WHERE r.receiver_state_id = ? AND r.source_id = ? \
               AND (r.device_id IN (?, ?) OR r.stream = 'workout') \
               AND r.state = 'applied' AND r.stream = ? \
             GROUP BY r.device_id",
        )
        .bind(&installation.receiver_state_id)
        .bind(&installation.source_id)
        .bind(IMPORTED_DEVICE_ID)
        .bind(COMPUTED_DEVICE_ID)
        .bind(stream.name())
        .fetch_all(pool)
        .await?;
        if latest.len() < devices.len() {
            return Ok(None);
        }
        for accepted_at in latest {
            let accepted_at = DateTime::parse_from_rfc3339(&accepted_at)
                .map_err(|err| {
                    anyhow::anyhow!("invalid batch_ledger.accepted_at {accepted_at:?}: {err}")
                })?
                .with_timezone(&Utc);
            oldest = Some(oldest.map_or(accepted_at, |oldest| oldest.min(accepted_at)));
        }
    }
    Ok(oldest)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Freshness {
    Confirmed,
    Partial,
    Unconfirmed,
    Unknown,
}

pub async fn freshness(
    pool: &SqlitePool,
    installation: &Installation,
    last_push_at: Option<DateTime<Utc>>,
    day: CalendarDay,
    rows: &[Rows],
) -> anyhow::Result<Freshness> {
    'cutoffs: for (since, freshness) in [
        (Some(day.end()), Freshness::Confirmed),
        (Some(day.start()), Freshness::Partial),
        (None, Freshness::Unconfirmed),
    ] {
        for &rows in rows {
            if coverage(pool, installation, rows, since).await? != Coverage::Covered {
                continue 'cutoffs;
            }
        }
        return Ok(freshness);
    }
    Ok(if last_push_at.is_some_and(|at| at < day.start()) {
        Freshness::Unconfirmed
    } else {
        Freshness::Unknown
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Coverage {
    Covered,
    Unknown,
}

#[derive(Debug, Serialize)]
pub struct DaySync<C> {
    pub freshness: Freshness,
    pub coverage: C,
}

#[derive(Debug, Serialize)]
pub struct NoopSync<C> {
    pub installation_id: Option<String>,
    pub imported_device_id: &'static str,
    pub computed_device_id: &'static str,
    pub last_push_at: Option<String>,
    pub freshness: Freshness,
    pub coverage: C,
}

impl<C> NoopSync<C> {
    pub const fn unsynced(coverage: C) -> Self {
        Self {
            installation_id: None,
            imported_device_id: IMPORTED_DEVICE_ID,
            computed_device_id: COMPUTED_DEVICE_ID,
            last_push_at: None,
            freshness: Freshness::Unknown,
            coverage,
        }
    }

    pub fn synced(
        installation: Installation,
        last_push_at: Option<DateTime<Utc>>,
        DaySync {
            freshness,
            coverage,
        }: DaySync<C>,
    ) -> Self {
        Self {
            installation_id: Some(installation.source_id),
            imported_device_id: IMPORTED_DEVICE_ID,
            computed_device_id: COMPUTED_DEVICE_ID,
            last_push_at: last_push_at.map(calendar::iso),
            freshness,
            coverage,
        }
    }
    pub fn map_coverage<D>(self, f: impl FnOnce(C) -> D) -> NoopSync<D> {
        NoopSync {
            installation_id: self.installation_id,
            imported_device_id: self.imported_device_id,
            computed_device_id: self.computed_device_id,
            last_push_at: self.last_push_at,
            freshness: self.freshness,
            coverage: f(self.coverage),
        }
    }
}

pub async fn coverage(
    pool: &SqlitePool,
    installation: &Installation,
    rows: Rows,
    since: Option<DateTime<Utc>>,
) -> anyhow::Result<Coverage> {
    match rows {
        Rows::DailyMetrics(day) => {
            day_coverage(pool, installation, Stream::DailyMetric, day, since).await
        }
        Rows::SleepSessions(day) => {
            let from = day.previous().start().timestamp();
            let to = day.end().timestamp();
            start_coverage(pool, installation, Stream::SleepSession, from, to, since).await
        }
        Rows::Workouts(day) => {
            let from = day.start().timestamp();
            let to = day.end().timestamp();
            start_coverage(pool, installation, Stream::Workout, from, to, since).await
        }
        Rows::Journal(day) => day_coverage(pool, installation, Stream::Journal, day, since).await,
    }
}

async fn day_coverage(
    pool: &SqlitePool,
    installation: &Installation,
    stream: Stream,
    day: CalendarDay,
    since: Option<DateTime<Utc>>,
) -> anyhow::Result<Coverage> {
    let devices = stream_devices(pool, installation, stream).await?;
    let day = day.to_string();
    let covering: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT r.device_id FROM replacement r \
         JOIN replacement_part p USING (receiver_state_id, source_id, device_id, replacement_id) \
         JOIN batch_ledger l USING (receiver_state_id, source_id, device_id, batch_id) \
         WHERE r.receiver_state_id = ? AND r.source_id = ? \
           AND r.stream = ? AND r.selector = 'day' AND r.state = 'applied' \
           AND r.start_inclusive <= ? AND r.end_exclusive > ? AND l.accepted_at >= ?",
    )
    .bind(&installation.receiver_state_id)
    .bind(&installation.source_id)
    .bind(stream.name())
    .bind(&day)
    .bind(&day)
    .bind(accepted_since(since))
    .fetch_all(pool)
    .await?;
    let covered = !devices.is_empty() && devices.iter().all(|device| covering.contains(device));
    Ok(if covered {
        Coverage::Covered
    } else {
        Coverage::Unknown
    })
}

async fn start_coverage(
    pool: &SqlitePool,
    installation: &Installation,
    stream: Stream,
    from: i64,
    to: i64,
    since: Option<DateTime<Utc>>,
) -> anyhow::Result<Coverage> {
    let devices = stream_devices(pool, installation, stream).await?;
    let windows: Vec<(String, i64, i64)> = sqlx::query_as(
        "SELECT DISTINCT r.device_id, CAST(r.start_inclusive AS INTEGER), \
                CAST(r.end_exclusive AS INTEGER) \
         FROM replacement r \
         JOIN replacement_part p USING (receiver_state_id, source_id, device_id, replacement_id) \
         JOIN batch_ledger l USING (receiver_state_id, source_id, device_id, batch_id) \
         WHERE r.receiver_state_id = ? AND r.source_id = ? \
           AND (r.device_id IN (?, ?) OR r.stream = 'workout') \
           AND r.stream = ? AND r.selector = 'startTs' AND r.state = 'applied' \
           AND CAST(r.start_inclusive AS INTEGER) < ? AND CAST(r.end_exclusive AS INTEGER) > ? \
           AND l.accepted_at >= ?",
    )
    .bind(&installation.receiver_state_id)
    .bind(&installation.source_id)
    .bind(IMPORTED_DEVICE_ID)
    .bind(COMPUTED_DEVICE_ID)
    .bind(stream.name())
    .bind(to)
    .bind(from)
    .bind(accepted_since(since))
    .fetch_all(pool)
    .await?;
    let covered = devices.iter().all(|device_id| {
        let intervals = windows
            .iter()
            .filter(|(window_device, ..)| window_device == device_id)
            .map(|&(_, start, end)| (start, end));
        spans(intervals, from, to)
    });
    Ok(if covered {
        Coverage::Covered
    } else {
        Coverage::Unknown
    })
}

fn accepted_since(since: Option<DateTime<Utc>>) -> String {
    since
        .map(|at| at.to_rfc3339_opts(SecondsFormat::Millis, true))
        .unwrap_or_default()
}

fn spans(intervals: impl Iterator<Item = (i64, i64)>, from: i64, to: i64) -> bool {
    let mut intervals: Vec<(i64, i64)> = intervals.filter(|(start, end)| start < end).collect();
    intervals.sort_unstable();
    let mut reached = from;
    for (start, end) in intervals {
        if reached >= to {
            break;
        }
        if start > reached {
            return false;
        }
        reached = reached.max(end);
    }
    reached >= to
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spans_requires_a_gapless_union() {
        assert!(spans([(0, 10)].into_iter(), 2, 8));
        assert!(spans([(5, 10), (0, 5)].into_iter(), 0, 10));
        assert!(spans([(0, 6), (3, 12)].into_iter(), 1, 11));
        assert!(!spans([(0, 4), (5, 10)].into_iter(), 0, 10));
        assert!(!spans([(1, 10)].into_iter(), 0, 10));
        assert!(!spans([(0, 9)].into_iter(), 0, 10));
        assert!(!spans(std::iter::empty(), 0, 10));
    }
}
