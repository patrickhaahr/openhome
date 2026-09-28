//! Recovery Day: one Copenhagen day's sleep and recovery values from the NOOP mirror.
//!
//! The Sleep Night is the one that wakes on the day: its sessions end inside the day's local
//! bounds, matching NOOP's daily rows, which are keyed by wake day. Every value carries its unit
//! and the namespace it was selected from, and the `noop` block says how far the receiver's copy
//! can be trusted, so a missing sync is never mistaken for a recorded null.

use serde::Serialize;
use sqlx::SqlitePool;

use crate::error::{AppError, Result};

use super::calendar::{self, CalendarDay, TIME_ZONE_NAME};
use super::noop_merge::{
    self, DailyRow, Namespace, ResolvedDaily, SessionRow, Sourced, StageMinutes,
};
use super::noop_source::{
    self, COMPUTED_DEVICE_ID, Coverage, IMPORTED_DEVICE_ID, Installation, NoopSync, Rows, Stream,
};

/// Sessions one Sleep Night may hold. A larger night is rejected rather than truncated.
pub const MAX_SLEEP_SESSIONS: usize = 24;

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
    pub derived_scores: DerivedScores,
    pub noop: NoopSync<NoopCoverage>,
}

/// A value with its unit and the NOOP device namespace it was selected from (`null` with it).
#[derive(Debug, Serialize)]
pub struct Measurement<T> {
    pub value: Option<T>,
    pub unit: &'static str,
    pub source: Option<&'static str>,
}

impl<T> Measurement<T> {
    fn new(sourced: Sourced<T>, unit: &'static str) -> Self {
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

#[derive(Debug, Serialize)]
pub struct NoopCoverage {
    pub daily_metrics: Coverage,
    pub sleep_sessions: Coverage,
}

const MINUTES: &str = "min";
const FRACTION: &str = "fraction";
const COUNT: &str = "count";
const BEATS_PER_MINUTE: &str = "beats/min";
const MILLISECONDS: &str = "ms";
const SCORE: &str = "score_0_100";

/// The streams a Recovery Day reads; their watermark is the Recovery Day's `last_push_at`.
const RECOVERY_STREAMS: [Stream; 2] = [Stream::DailyMetric, Stream::SleepSession];

pub async fn recovery_day(pool: &SqlitePool, day: CalendarDay) -> Result<RecoveryDay> {
    let Some(installation) = noop_source::active_installation(pool).await? else {
        let unsynced = NoopSync::unsynced(NoopCoverage {
            daily_metrics: Coverage::Unknown,
            sleep_sessions: Coverage::Unknown,
        });
        return Ok(assemble(
            day,
            &noop_merge::merge_daily(None, None, false),
            Vec::new(),
            unsynced,
        ));
    };

    let rows = [Rows::DailyMetrics(day), Rows::SleepSessions(day)];
    let last_push_at = noop_source::last_push_at(pool, &installation, &RECOVERY_STREAMS).await?;
    let coverage = NoopCoverage {
        daily_metrics: noop_source::coverage(pool, &installation, rows[0], None).await?,
        sleep_sessions: noop_source::coverage(pool, &installation, rows[1], None).await?,
    };
    let freshness = noop_source::freshness(pool, &installation, last_push_at, day, &rows).await?;
    let sessions = sleep_sessions(pool, &installation, day).await?;
    let sleep_edited = sessions
        .iter()
        .any(|session| session.namespace == Namespace::Computed && session.user_edited);
    let daily = daily_metrics(pool, &installation, day, sleep_edited).await?;
    let sessions = noop_merge::merge_sleep_sessions(sessions)
        .iter()
        .map(sleep_session)
        .collect::<Result<_>>()?;
    let sync = NoopSync::synced(installation, last_push_at, freshness, coverage);
    Ok(assemble(day, &daily, sessions, sync))
}

fn assemble(
    day: CalendarDay,
    daily: &ResolvedDaily,
    sessions: Vec<SleepSession>,
    noop: NoopSync<NoopCoverage>,
) -> RecoveryDay {
    RecoveryDay {
        day: day.to_string(),
        time_zone: TIME_ZONE_NAME,
        day_start: calendar::iso(day.start()),
        day_end: calendar::iso(day.end()),
        sleep: Sleep {
            total: Measurement::new(daily.total_sleep_min, MINUTES),
            deep: Measurement::new(daily.deep_min, MINUTES),
            rem: Measurement::new(daily.rem_min, MINUTES),
            light: Measurement::new(daily.light_min, MINUTES),
            efficiency: Measurement::new(daily.efficiency, FRACTION),
            disturbances: Measurement::new(daily.disturbances, COUNT),
            sessions,
        },
        resting_hr: Measurement::new(daily.resting_hr, BEATS_PER_MINUTE),
        hrv_rmssd: Measurement::new(daily.avg_hrv, MILLISECONDS),
        derived_scores: DerivedScores {
            recovery: Measurement::new(daily.recovery, SCORE),
            strain: Measurement::new(daily.strain, SCORE),
        },
        noop,
    }
}

async fn daily_metrics(
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
                disturbances, resting_hr, avg_hrv, recovery, strain \
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
