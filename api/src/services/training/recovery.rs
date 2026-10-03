//! Recovery Day: one Copenhagen day's sleep and recovery values from the NOOP mirror.
//!
//! The Sleep Night is the one that wakes on the day: its sessions end inside the day's local
//! bounds, matching NOOP's daily rows, which are keyed by wake day. Every value carries its unit
//! and the namespace it was selected from, and the `noop` block says how far the receiver's copy
//! can be trusted, so a missing sync is never mistaken for a recorded null.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::SqlitePool;

use crate::error::{AppError, Result};

use super::calendar::{self, CalendarDay, TIME_ZONE_NAME};
use super::noop_merge::{
    self, DailyRow, Namespace, ResolvedDaily, SessionRow, Sourced, StageMinutes,
};
use super::noop_source::{
    self, COMPUTED_DEVICE_ID, Coverage, DaySync, IMPORTED_DEVICE_ID, Installation, NoopSync, Rows,
    Stream,
};

/// Sessions one Sleep Night may hold. A larger night is rejected rather than truncated.
pub const MAX_SLEEP_SESSIONS: usize = 24;
/// Journal rows one day may hold across namespaces, before one entry per question is kept. A
/// larger journal is rejected rather than truncated.
pub const MAX_JOURNAL_ROWS: usize = 64;

#[derive(Debug, Serialize)]
pub struct RecoveryDay {
    pub day: String,
    pub time_zone: &'static str,
    /// Local midnight at the start of the day.
    pub day_start: String,
    /// Local midnight at the end of the day (exclusive).
    pub day_end: String,
    pub sleep: Sleep,
    pub resting_hr: Measurement<f64>,
    /// Nightly heart-rate variability as RMSSD.
    pub hrv_rmssd: Measurement<f64>,
    /// Nightly skin temperature as a deviation from the user's baseline, not an absolute temperature.
    pub skin_temp_deviation: Measurement<f64>,
    pub respiratory_rate: Measurement<f64>,
    pub derived_scores: DerivedScores,
    /// The Journal Entries logged against the day, by question.
    pub journal: Vec<JournalEntry>,
    pub noop: NoopSync<RecoveryCoverage>,
}

/// A value with its unit and the NOOP device namespace it was selected from (`null` with it).
#[derive(Debug, Serialize)]
pub struct Measurement<T> {
    pub value: Option<T>,
    pub unit: &'static str,
    pub source: Option<&'static str>,
}

impl<T> Measurement<T> {
    pub(super) fn new(sourced: Sourced<T>, unit: &'static str) -> Self {
        Self {
            value: sourced.value,
            unit,
            source: sourced.source.map(Namespace::device_id),
        }
    }
}

/// NOOP's daily sleep summary for the night plus the sessions it was drawn from.
#[derive(Debug, Serialize)]
pub struct Sleep {
    pub total: Measurement<f64>,
    pub deep: Measurement<f64>,
    pub rem: Measurement<f64>,
    pub light: Measurement<f64>,
    pub efficiency: Measurement<f64>,
    pub disturbances: Measurement<i64>,
    pub sessions: Vec<SleepSession>,
}

#[derive(Debug, Serialize)]
pub struct SleepSession {
    /// The start NOOP shows: the user's adjusted start when edited, else the detected start.
    pub start: String,
    pub end: String,
    pub detected_start: String,
    pub duration_min: f64,
    pub stage_min: Option<StageMinutes>,
    pub efficiency_fraction: Option<f64>,
    pub resting_hr_bpm: Option<f64>,
    pub hrv_rmssd_ms: Option<f64>,
    pub user_edited: bool,
    pub staging_sparse: Option<bool>,
    pub source: &'static str,
}

/// Scores NOOP derives from the strap data; observations of NOOP's model, not measurements.
#[derive(Debug, Serialize)]
pub struct DerivedScores {
    pub recovery: Measurement<f64>,
    pub strain: Measurement<f64>,
}

/// One answer the user logged in NOOP against the day; never shifted to another day.
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct JournalEntry {
    pub question: String,
    pub answered_yes: bool,
    pub numeric_value: Option<f64>,
    pub notes: Option<String>,
    /// The NOOP device namespace the entry was read from.
    #[sqlx(rename = "device_id")]
    pub source: String,
}

/// Coverage of the streams behind a Sleep Night and its daily values.
#[derive(Debug, Serialize)]
pub struct NoopCoverage {
    pub daily_metrics: Coverage,
    pub sleep_sessions: Coverage,
}

/// Coverage of every stream a Recovery Day reads.
#[derive(Debug, Serialize)]
pub struct RecoveryCoverage {
    #[serde(flatten)]
    pub night: NoopCoverage,
    pub journal: Coverage,
}

/// A Recovery Day without its Journal Entries: the Sleep Night waking on the day, the day's
/// resolved daily values, and the sync state of both.
pub(super) struct Night {
    pub sleep: Sleep,
    pub daily: ResolvedDaily,
    pub noop: NoopSync<NoopCoverage>,
    pub installation: Option<Installation>,
}

pub(super) const MINUTES: &str = "min";
const FRACTION: &str = "fraction";
const COUNT: &str = "count";
pub(super) const BEATS_PER_MINUTE: &str = "beats/min";
pub(super) const MILLISECONDS: &str = "ms";
const SCORE: &str = "score_0_100";
const DEGREES_CELSIUS: &str = "degC";
const BREATHS_PER_MINUTE: &str = "breaths/min";

/// The streams a Recovery Day reads; their watermark is the Recovery Day's `last_push_at`.
pub(super) const RECOVERY_STREAMS: [Stream; 2] = [Stream::DailyMetric, Stream::SleepSession];

pub async fn recovery_day(pool: &SqlitePool, day: CalendarDay) -> Result<RecoveryDay> {
    let Night {
        sleep,
        daily,
        noop,
        installation,
    } = night(pool, day).await?;
    let (journal, journal_coverage) = match &installation {
        Some(installation) => (
            journal_entries(pool, installation, day).await?,
            noop_source::coverage(pool, installation, Rows::Journal(day), None).await?,
        ),
        None => (Vec::new(), Coverage::Unknown),
    };
    Ok(RecoveryDay {
        day: day.to_string(),
        time_zone: TIME_ZONE_NAME,
        day_start: calendar::iso(day.start()),
        day_end: calendar::iso(day.end()),
        sleep,
        resting_hr: Measurement::new(daily.resting_hr, BEATS_PER_MINUTE),
        hrv_rmssd: Measurement::new(daily.avg_hrv, MILLISECONDS),
        skin_temp_deviation: Measurement::new(daily.skin_temp_dev_c, DEGREES_CELSIUS),
        respiratory_rate: Measurement::new(daily.resp_rate_bpm, BREATHS_PER_MINUTE),
        derived_scores: DerivedScores {
            recovery: Measurement::new(daily.recovery, SCORE),
            strain: Measurement::new(daily.strain, SCORE),
        },
        journal,
        noop: noop.map_coverage(|night| RecoveryCoverage {
            night,
            journal: journal_coverage,
        }),
    })
}

/// The Sleep Night waking on `day` with the day's daily values. Journal Entries are not read, so
/// their pushes and bounds cannot affect it.
pub(super) async fn night(pool: &SqlitePool, day: CalendarDay) -> Result<Night> {
    let Some(installation) = noop_source::active_installation(pool).await? else {
        let daily = noop_merge::merge_daily(None, None, false);
        return Ok(Night {
            sleep: sleep(&daily, Vec::new()),
            daily,
            noop: NoopSync::unsynced(NoopCoverage {
                daily_metrics: Coverage::Unknown,
                sleep_sessions: Coverage::Unknown,
            }),
            installation: None,
        });
    };

    let last_push_at = noop_source::last_push_at(pool, &installation, &RECOVERY_STREAMS).await?;
    let day_sync = day_sync(pool, &installation, last_push_at, day).await?;
    let sessions = sleep_sessions(pool, &installation, day).await?;
    let sleep_edited = sessions
        .iter()
        .any(|session| session.namespace == Namespace::Computed && session.user_edited);
    let daily = daily_metrics(pool, &installation, day, sleep_edited).await?;
    let sessions = noop_merge::merge_sleep_sessions(sessions)
        .iter()
        .map(sleep_session)
        .collect::<Result<_>>()?;
    Ok(Night {
        sleep: sleep(&daily, sessions),
        daily,
        noop: NoopSync::synced(installation.clone(), last_push_at, day_sync),
        installation: Some(installation),
    })
}

/// Freshness and coverage of the rows a Recovery Day of `day` reads. `last_push_at` is the
/// watermark of [`RECOVERY_STREAMS`].
pub(super) async fn day_sync(
    pool: &SqlitePool,
    installation: &Installation,
    last_push_at: Option<DateTime<Utc>>,
    day: CalendarDay,
) -> Result<DaySync<NoopCoverage>> {
    let rows = [Rows::DailyMetrics(day), Rows::SleepSessions(day)];
    Ok(DaySync {
        freshness: noop_source::freshness(pool, installation, last_push_at, day, &rows).await?,
        coverage: NoopCoverage {
            daily_metrics: noop_source::coverage(pool, installation, rows[0], None).await?,
            sleep_sessions: noop_source::coverage(pool, installation, rows[1], None).await?,
        },
    })
}

fn sleep(daily: &ResolvedDaily, sessions: Vec<SleepSession>) -> Sleep {
    Sleep {
        total: Measurement::new(daily.total_sleep_min, MINUTES),
        deep: Measurement::new(daily.deep_min, MINUTES),
        rem: Measurement::new(daily.rem_min, MINUTES),
        light: Measurement::new(daily.light_min, MINUTES),
        efficiency: Measurement::new(daily.efficiency, FRACTION),
        disturbances: Measurement::new(daily.disturbances, COUNT),
        sessions,
    }
}

/// Whether a computed session waking on this Copenhagen day was edited in NOOP. Trends use this
/// bounded lookup because they do not fetch the full sessions that a Recovery Day already has.
pub(super) async fn sleep_edited(
    pool: &SqlitePool,
    installation: &Installation,
    day: CalendarDay,
) -> Result<bool> {
    let edited: i64 = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM sleep_session WHERE source_id = ? AND device_id = ? \
         AND end_ts >= ? AND end_ts < ? AND user_edited = 1)",
    )
    .bind(&installation.source_id)
    .bind(COMPUTED_DEVICE_ID)
    .bind(day.start().timestamp())
    .bind(day.end().timestamp())
    .fetch_one(pool)
    .await
    .map_err(anyhow::Error::from)?;
    Ok(edited != 0)
}

pub(super) async fn daily_metrics(
    pool: &SqlitePool,
    installation: &Installation,
    day: CalendarDay,
    sleep_edited: bool,
) -> Result<ResolvedDaily> {
    #[derive(sqlx::FromRow)]
    struct Record {
        device_id: String,
        #[sqlx(flatten)]
        metrics: DailyRow,
    }

    let records: Vec<Record> = sqlx::query_as(
        "SELECT device_id, total_sleep_min, efficiency, deep_min, rem_min, light_min, \
                disturbances, resting_hr, avg_hrv, recovery, strain, skin_temp_dev_c, \
                resp_rate_bpm \
         FROM daily_metric WHERE source_id = ? AND device_id IN (?, ?) AND day = ?",
    )
    .bind(&installation.source_id)
    .bind(IMPORTED_DEVICE_ID)
    .bind(COMPUTED_DEVICE_ID)
    .bind(day.to_string())
    .fetch_all(pool)
    .await
    .map_err(anyhow::Error::from)?;
    let row = |namespace: Namespace| {
        records
            .iter()
            .find(|record| record.device_id == namespace.device_id())
            .map(|record| &record.metrics)
    };
    Ok(noop_merge::merge_daily(
        row(Namespace::Imported),
        row(Namespace::Computed),
        sleep_edited,
    ))
}

/// Both namespaces' sessions that end on `day`, before source selection.
async fn sleep_sessions(
    pool: &SqlitePool,
    installation: &Installation,
    day: CalendarDay,
) -> Result<Vec<SessionRow>> {
    #[derive(sqlx::FromRow)]
    struct Record {
        device_id: String,
        start_ts: i64,
        end_ts: i64,
        efficiency: Option<f64>,
        resting_hr: Option<f64>,
        avg_hrv: Option<f64>,
        stages_json: Option<String>,
        user_edited: bool,
        start_ts_adjusted: Option<i64>,
        staging_sparse: Option<bool>,
    }

    let records: Vec<Record> = sqlx::query_as(
        "SELECT device_id, start_ts, end_ts, efficiency, resting_hr, avg_hrv, stages_json, \
                user_edited, start_ts_adjusted, staging_sparse \
         FROM sleep_session \
         WHERE source_id = ? AND device_id IN (?, ?) AND end_ts >= ? AND end_ts < ? \
         ORDER BY device_id, start_ts LIMIT ?",
    )
    .bind(&installation.source_id)
    .bind(IMPORTED_DEVICE_ID)
    .bind(COMPUTED_DEVICE_ID)
    .bind(day.start().timestamp())
    .bind(day.end().timestamp())
    .bind(MAX_SLEEP_SESSIONS as i64 + 1)
    .fetch_all(pool)
    .await
    .map_err(anyhow::Error::from)?;
    if records.len() > MAX_SLEEP_SESSIONS {
        return Err(AppError::Unprocessable(format!(
            "The Sleep Night waking on {day} has more than {MAX_SLEEP_SESSIONS} sleep sessions; \
             refusing to return a truncated night"
        )));
    }
    Ok(records
        .into_iter()
        .filter_map(|record| {
            Some(SessionRow {
                namespace: Namespace::from_device_id(&record.device_id)?,
                start_ts: record.start_ts,
                end_ts: record.end_ts,
                efficiency: record.efficiency,
                resting_hr: record.resting_hr,
                avg_hrv: record.avg_hrv,
                stages_json: record.stages_json,
                user_edited: record.user_edited,
                start_ts_adjusted: record.start_ts_adjusted,
                staging_sparse: record.staging_sparse,
            })
        })
        .collect())
}

/// The Journal Entries recorded against `day`, one per question across namespaces.
async fn journal_entries(
    pool: &SqlitePool,
    installation: &Installation,
    day: CalendarDay,
) -> Result<Vec<JournalEntry>> {
    let mut entries: Vec<JournalEntry> = sqlx::query_as(
        "SELECT question, answered_yes, numeric_value, notes, device_id FROM journal \
         WHERE source_id = ? AND day = ? LIMIT ?",
    )
    .bind(&installation.source_id)
    .bind(day.to_string())
    .bind(MAX_JOURNAL_ROWS as i64 + 1)
    .fetch_all(pool)
    .await
    .map_err(anyhow::Error::from)?;
    if entries.len() > MAX_JOURNAL_ROWS {
        return Err(AppError::Unprocessable(format!(
            "More than {MAX_JOURNAL_ROWS} Journal Entry rows are logged against {day}; \
             refusing to return a truncated journal"
        )));
    }
    entries.sort_by(|a, b| {
        a.question.cmp(&b.question).then_with(|| {
            noop_merge::journal_precedence(&a.source)
                .cmp(&noop_merge::journal_precedence(&b.source))
        })
    });
    entries.dedup_by(|later, kept| later.question == kept.question);
    Ok(entries)
}

fn sleep_session(session: &SessionRow) -> Result<SleepSession> {
    let start = session.effective_start_ts();
    Ok(SleepSession {
        start: calendar::iso_from_unix(start)?,
        end: calendar::iso_from_unix(session.end_ts)?,
        detected_start: calendar::iso_from_unix(session.start_ts)?,
        duration_min: (session.end_ts - start) as f64 / 60.0,
        stage_min: session
            .stages_json
            .as_deref()
            .and_then(noop_merge::stage_minutes),
        efficiency_fraction: session.efficiency,
        resting_hr_bpm: session.resting_hr,
        hrv_rmssd_ms: session.avg_hrv,
        user_edited: session.user_edited,
        staging_sparse: session.staging_sparse,
        source: session.namespace.device_id(),
    })
}
