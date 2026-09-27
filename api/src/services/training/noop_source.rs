//! Which NOOP installation and namespaces a read uses, and how current the receiver's copy is.
//!
//! NOOP writes one strap's data under two device namespaces of the same installation: the strap's
//! imported values and NOOP's on-device computed values. Both are read together and resolved with
//! NOOP's precedence rules ([`super::noop_merge`]).
//!
//! Freshness and coverage come only from the receiver's push bookkeeping (`batch_ledger` and
//! applied `replacement` windows under the current receiver state), never from the presence of
//! rows: an empty result is a recorded absence only when an applied window covers it.

use chrono::{DateTime, SecondsFormat, Utc};
use serde::Serialize;
use sqlx::SqlitePool;

use super::calendar::CalendarDay;

/// NOOP's canonical namespace for the strap's imported values. NOOP names computed values
/// `"<strap>-noop"`; a strap re-added under another id is not resolved.
pub const IMPORTED_DEVICE_ID: &str = "my-whoop";
/// NOOP's on-device computed sibling of [`IMPORTED_DEVICE_ID`]; not a second strap.
pub const COMPUTED_DEVICE_ID: &str = "my-whoop-noop";

const DAILY_METRIC_STREAM: &str = "dailyMetric";
const SLEEP_SESSION_STREAM: &str = "sleepSession";

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

/// The oldest of the latest completed daily-metric and sleep replacements in both namespaces.
/// A replacement completes when its last part is accepted. Staged parts and unrelated streams
/// cannot advance this watermark; it is unknown until all four scopes have completed a window.
pub async fn last_push_at(
    pool: &SqlitePool,
    installation: &Installation,
) -> anyhow::Result<Option<DateTime<Utc>>> {
    let latest: Vec<String> = sqlx::query_scalar(
        "SELECT MAX(l.accepted_at) FROM replacement r \
         JOIN replacement_part p USING (receiver_state_id, source_id, device_id, replacement_id) \
         JOIN batch_ledger l USING (receiver_state_id, source_id, device_id, batch_id) \
         WHERE r.receiver_state_id = ? AND r.source_id = ? AND r.device_id IN (?, ?) \
           AND r.state = 'applied' AND r.stream IN ('dailyMetric', 'sleepSession') \
         GROUP BY r.device_id, r.stream",
    )
    .bind(&installation.receiver_state_id)
    .bind(&installation.source_id)
    .bind(IMPORTED_DEVICE_ID)
    .bind(COMPUTED_DEVICE_ID)
    .fetch_all(pool)
    .await?;
    if latest.len() < 4 {
        return Ok(None);
    }
    let mut oldest: Option<DateTime<Utc>> = None;
    for accepted_at in latest {
        let accepted_at = DateTime::parse_from_rfc3339(&accepted_at)
            .map_err(|err| {
                anyhow::anyhow!("invalid batch_ledger.accepted_at {accepted_at:?}: {err}")
            })?
            .with_timezone(&Utc);
        oldest = Some(oldest.map_or(accepted_at, |oldest| oldest.min(accepted_at)));
    }
    Ok(oldest)
}

/// How recently completed recovery windows cover the requested day and Sleep Night.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Freshness {
    /// Both streams and namespaces have coverage completed after the day ended.
    Confirmed,
    /// Complete coverage arrived during the day, but not after its end.
    Partial,
    /// Complete coverage or the recovery watermark predates the day.
    Unconfirmed,
    /// The receiver cannot establish completed recovery coverage for the day.
    Unknown,
}

pub async fn freshness(
    pool: &SqlitePool,
    installation: &Installation,
    last_push_at: Option<DateTime<Utc>>,
    day: CalendarDay,
) -> anyhow::Result<Freshness> {
    for (since, freshness) in [
        (Some(day.end()), Freshness::Confirmed),
        (Some(day.start()), Freshness::Partial),
        (None, Freshness::Unconfirmed),
    ] {
        if daily_metric_coverage(pool, installation, day, since).await? == Coverage::Covered
            && sleep_session_coverage(pool, installation, day, since).await? == Coverage::Covered
        {
            return Ok(freshness);
        }
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

/// `dailyMetric` rows for `day`: covered when, in both namespaces, some applied window contains
/// the day. Day windows use canonical `YYYY-MM-DD` text, so text comparison orders them.
/// With `since`, only windows completed at or after that instant count.
pub async fn daily_metric_coverage(
    pool: &SqlitePool,
    installation: &Installation,
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
    .bind(DAILY_METRIC_STREAM)
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

/// `sleepSession` rows of the Sleep Night that wakes on `day`: covered when, in both namespaces,
/// applied windows jointly span session starts from the previous local midnight to the end of the
/// day. Assumes a sleep session is shorter than a day.
/// With `since`, only windows completed at or after that instant count.
pub async fn sleep_session_coverage(
    pool: &SqlitePool,
    installation: &Installation,
    day: CalendarDay,
    since: Option<DateTime<Utc>>,
) -> anyhow::Result<Coverage> {
    let from = day.previous().start().timestamp();
    let to = day.end().timestamp();
    let windows: Vec<(String, i64, i64)> = sqlx::query_as(
        "SELECT DISTINCT r.device_id, CAST(r.start_inclusive AS INTEGER), \
                CAST(r.end_exclusive AS INTEGER) \
         FROM replacement r \
         JOIN replacement_part p USING (receiver_state_id, source_id, device_id, replacement_id) \
         JOIN batch_ledger l USING (receiver_state_id, source_id, device_id, batch_id) \
         WHERE r.receiver_state_id = ? AND r.source_id = ? AND r.device_id IN (?, ?) \
           AND r.stream = ? AND r.selector = 'startTs' AND r.state = 'applied' \
           AND CAST(r.start_inclusive AS INTEGER) < ? AND CAST(r.end_exclusive AS INTEGER) > ? \
           AND l.accepted_at >= ?",
    )
    .bind(&installation.receiver_state_id)
    .bind(&installation.source_id)
    .bind(IMPORTED_DEVICE_ID)
    .bind(COMPUTED_DEVICE_ID)
    .bind(SLEEP_SESSION_STREAM)
    .bind(to)
    .bind(from)
    .bind(accepted_since(since))
    .fetch_all(pool)
    .await?;
    let covered = [IMPORTED_DEVICE_ID, COMPUTED_DEVICE_ID]
        .iter()
        .all(|device_id| {
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
