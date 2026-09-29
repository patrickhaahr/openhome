//! Which NOOP installation and namespaces a read uses, and how current the receiver's copy is.
//!
//! NOOP writes one strap's data under two device namespaces of the same installation: the strap's
//! imported values and NOOP's on-device computed values. Both are read together and resolved with
//! NOOP's precedence rules ([`super::noop_merge`]).
//!
//! Freshness and coverage come only from the receiver's push bookkeeping (`batch_ledger` and
//! applied `replacement` windows under the current receiver state), never from the presence of
//! rows: an empty result is a recorded absence only when an applied window covers it. Each read
//! names the replace-window [`Rows`] it depends on and gets freshness and coverage for exactly those.

use chrono::{DateTime, SecondsFormat, Utc};
use serde::Serialize;
use sqlx::SqlitePool;

use super::calendar::{self, CalendarDay};

/// NOOP's canonical namespace for the strap's imported values. NOOP names computed values
/// `"<strap>-noop"`; a strap re-added under another id is not resolved.
pub const IMPORTED_DEVICE_ID: &str = "my-whoop";
/// NOOP's on-device computed sibling of [`IMPORTED_DEVICE_ID`]; not a second strap.
pub const COMPUTED_DEVICE_ID: &str = "my-whoop-noop";

/// A mutable NOOP stream delivered as replacement windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stream {
    DailyMetric,
    SleepSession,
    Workout,
}

impl Stream {
    const fn name(self) -> &'static str {
        match self {
            Self::DailyMetric => "dailyMetric",
            Self::SleepSession => "sleepSession",
            Self::Workout => "workout",
        }
    }
}

/// The mirrored rows a read of one day depends on.
#[derive(Debug, Clone, Copy)]
pub enum Rows {
    /// `dailyMetric` rows keyed by the day.
    DailyMetrics(CalendarDay),
    /// `sleepSession` rows of the Sleep Night that wakes on the day. Their starts can fall on the
    /// previous day; this assumes a sleep session is shorter than a day.
    SleepSessions(CalendarDay),
    /// `workout` rows starting on the day.
    Workouts(CalendarDay),
}

/// The installation whose pushes are current: the sender of the latest strap batch accepted under
/// the current receiver state. `None` until such a push arrives (including after a rotation).
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

/// Namespaces that must be accounted for before declaring a stream complete. Workout imports can
/// have their own device IDs; retained rows and even staged windows establish that such a source
/// exists, while only applied windows can establish its coverage.
async fn stream_devices(
    pool: &SqlitePool,
    installation: &Installation,
    stream: Stream,
) -> anyhow::Result<Vec<String>> {
    if stream != Stream::Workout {
        return Ok(vec![
            IMPORTED_DEVICE_ID.to_owned(),
            COMPUTED_DEVICE_ID.to_owned(),
        ]);
    }
    Ok(sqlx::query_scalar(
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
    .await?)
}

/// The oldest of the latest completed replacements of `streams` in their namespaces. A replacement
/// completes when its last part is accepted. Staged parts and other streams cannot advance this
/// watermark; it is unknown until every stream has completed a window in every relevant namespace.
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

/// How recently completed windows cover the rows a read of one day depends on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Freshness {
    /// Every stream and namespace has coverage completed after the day ended.
    Confirmed,
    /// Complete coverage arrived during the day, but not after its end.
    Partial,
    /// Complete coverage or the read's push watermark predates the day.
    Unconfirmed,
    /// The receiver cannot establish completed coverage for the day.
    Unknown,
}

/// `rows` are the rows the read of `day` depends on; `last_push_at` is the watermark of their
/// streams.
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

/// Whether applied replacement windows establish the receiver's rows for a stream and day. Only
/// `covered` makes missing rows a recorded absence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Coverage {
    Covered,
    Unknown,
}

/// How far the receiver's copy of a read's rows can be trusted. `C` names the coverage of each
/// stream the read uses.
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
    /// No strap push has arrived under the current receiver state.
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
        freshness: Freshness,
        coverage: C,
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
}

/// Whether applied windows establish `rows` in every relevant namespace. With `since`, only windows
/// completed at or after that instant count.
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
    }
}

/// Day-keyed rows: covered when, in both namespaces, some applied window contains the day. Day
/// windows use canonical `YYYY-MM-DD` text, so text comparison orders them.
async fn day_coverage(
    pool: &SqlitePool,
    installation: &Installation,
    stream: Stream,
    day: CalendarDay,
    since: Option<DateTime<Utc>>,
) -> anyhow::Result<Coverage> {
    let day = day.to_string();
    let covering: i64 = sqlx::query_scalar(
        "SELECT COUNT(DISTINCT r.device_id) FROM replacement r \
         JOIN replacement_part p USING (receiver_state_id, source_id, device_id, replacement_id) \
         JOIN batch_ledger l USING (receiver_state_id, source_id, device_id, batch_id) \
         WHERE r.receiver_state_id = ? AND r.source_id = ? AND r.device_id IN (?, ?) \
           AND r.stream = ? AND r.selector = 'day' AND r.state = 'applied' \
           AND r.start_inclusive <= ? AND r.end_exclusive > ? AND l.accepted_at >= ?",
    )
    .bind(&installation.receiver_state_id)
    .bind(&installation.source_id)
    .bind(IMPORTED_DEVICE_ID)
    .bind(COMPUTED_DEVICE_ID)
    .bind(stream.name())
    .bind(&day)
    .bind(&day)
    .bind(accepted_since(since))
    .fetch_one(pool)
    .await?;
    Ok(if covering == 2 {
        Coverage::Covered
    } else {
        Coverage::Unknown
    })
}

/// `startTs`-keyed rows starting in `[from, to)` (Unix seconds): covered when, in every namespace,
/// applied windows jointly span that interval.
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
    // The ledger writes fixed-width UTC timestamps with milliseconds, so lexical order is time
    // order. Any accepted part after the cutoff proves an applied window completed after it.
    since
        .map(|at| at.to_rfc3339_opts(SecondsFormat::Millis, true))
        .unwrap_or_default()
}

/// Whether the union of half-open intervals contains all of `[from, to)`.
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
