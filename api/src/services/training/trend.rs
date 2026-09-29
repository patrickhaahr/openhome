//! Bounded daily measurements and ISO-week means. Missing days stay null and do not enter means.

use serde::Serialize;
use sqlx::SqlitePool;

use crate::error::{AppError, Result};

use super::calendar::{self, CalendarDay, TIME_ZONE_NAME};
use super::noop_merge::{Namespace, ResolvedDaily, Sourced};
use super::noop_source::{self, Coverage, Freshness, Rows, Stream};
use super::recovery;

/// The four metrics agreed for the initial trend contract.
#[derive(Debug, Clone, Copy)]
pub enum Metric {
    BodyWeight,
    Noop(NoopMetric),
}

#[derive(Debug, Clone, Copy)]
pub enum NoopMetric {
    SleepDuration,
    RestingHeartRate,
    Hrv,
}

impl Metric {
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "body_weight" => Some(Self::BodyWeight),
            "sleep_duration" => Some(Self::Noop(NoopMetric::SleepDuration)),
            "resting_heart_rate" => Some(Self::Noop(NoopMetric::RestingHeartRate)),
            "hrv" => Some(Self::Noop(NoopMetric::Hrv)),
            _ => None,
        }
    }

    const fn name(self) -> &'static str {
        match self {
            Self::BodyWeight => "body_weight",
            Self::Noop(NoopMetric::SleepDuration) => "sleep_duration",
            Self::Noop(NoopMetric::RestingHeartRate) => "resting_heart_rate",
            Self::Noop(NoopMetric::Hrv) => "hrv",
        }
    }

    const fn unit(self) -> &'static str {
        match self {
            Self::BodyWeight => "kg",
            Self::Noop(metric) => metric.unit(),
        }
    }
}

impl NoopMetric {
    const fn unit(self) -> &'static str {
        match self {
            Self::SleepDuration => "min",
            Self::RestingHeartRate => "beats/min",
            Self::Hrv => "ms",
        }
    }

    const fn reads_sleep(self) -> bool {
        matches!(self, Self::SleepDuration)
    }

    const fn select(self, daily: &ResolvedDaily) -> Sourced<f64> {
        match self {
            Self::SleepDuration => daily.total_sleep_min,
            Self::RestingHeartRate => daily.resting_hr,
            Self::Hrv => daily.avg_hrv,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct MetricTrend {
    pub metric: &'static str,
    pub from_day: String,
    pub to_day: String,
    pub time_zone: &'static str,
    pub unit: &'static str,
    /// NOOP installation and watermark are null for daily Body Weight or before the first push.
    pub installation_id: Option<String>,
    pub last_push_at: Option<String>,
    pub daily_points: Vec<DailyPoint>,
    pub weekly_summaries: Vec<WeeklySummary>,
}

#[derive(Debug, Serialize)]
pub struct DailyPoint {
    pub day: String,
    pub value: Option<f64>,
    pub unit: &'static str,
    /// `body_weight` for a daily log entry; a NOOP namespace for mirrored values.
    pub source: Option<&'static str>,
    /// Null for Body Weight, which is entered directly into `OpenHome`.
    pub freshness: Option<Freshness>,
    pub coverage: Option<Coverage>,
}

#[derive(Debug, Serialize)]
pub struct WeeklySummary {
    /// Monday and Sunday of the complete ISO week, even for a partial requested week.
    pub week_start: String,
    pub week_end: String,
    /// First and last days of this week inside the requested interval.
    pub range_start: String,
    pub range_end: String,
    pub calendar_days: usize,
    pub observed_days: usize,
    /// Arithmetic mean of observed daily values; null when no day was measured.
    pub mean: Option<f64>,
    pub unit: &'static str,
}

struct WeekAccumulator {
    summary: WeeklySummary,
    sum: f64,
}

impl WeekAccumulator {
    fn new(
        day: CalendarDay,
        week_start: CalendarDay,
        week_end: CalendarDay,
        unit: &'static str,
    ) -> Self {
        Self {
            summary: WeeklySummary {
                week_start: week_start.to_string(),
                week_end: week_end.to_string(),
                range_start: day.to_string(),
                range_end: day.to_string(),
                calendar_days: 0,
                observed_days: 0,
                mean: None,
                unit,
            },
            sum: 0.0,
        }
    }

    fn add(&mut self, day: CalendarDay, value: Option<f64>) {
        self.summary.range_end = day.to_string();
        self.summary.calendar_days += 1;
        if let Some(value) = value {
            self.summary.observed_days += 1;
            self.sum += value;
        }
    }

    fn finish(mut self) -> WeeklySummary {
        if self.summary.observed_days > 0 {
            self.summary.mean = Some(self.sum / self.summary.observed_days as f64);
        }
        self.summary
    }
}

pub async fn metric_trend(
    db: &SqlitePool,
    noop_db: &SqlitePool,
    metric: Metric,
    from_day: CalendarDay,
    to_day: CalendarDay,
) -> Result<MetricTrend> {
    let unit = metric.unit();
    let mut daily_points = Vec::new();
    let mut installation_id = None;
    let mut last_push_at = None;

    match metric {
        Metric::BodyWeight => {
            let from = from_day.to_string();
            let to = to_day.to_string();
            let weights = sqlx::query!(
                r#"SELECT CAST(date AS TEXT) AS "day!: String", weight_kg
               FROM body_weight WHERE date BETWEEN $1 AND $2 ORDER BY date"#,
                from,
                to,
            )
            .fetch_all(db)
            .await
            .map_err(anyhow::Error::from)?;
            let mut weights = weights.iter().peekable();
            for day in days(from_day, to_day) {
                let value = if weights.peek().is_some_and(|row| row.day == day.to_string()) {
                    weights.next().map(|row| row.weight_kg)
                } else {
                    None
                };
                daily_points.push(DailyPoint {
                    day: day.to_string(),
                    value,
                    unit,
                    source: value.map(|_| "body_weight"),
                    freshness: None,
                    coverage: None,
                });
            }
        }
        Metric::Noop(noop_metric) => {
            if let Some(installation) = noop_source::active_installation(noop_db).await? {
                installation_id = Some(installation.source_id.clone());
                let streams: &[Stream] = if noop_metric.reads_sleep() {
                    &[Stream::DailyMetric, Stream::SleepSession]
                } else {
                    &[Stream::DailyMetric]
                };
                let pushed = noop_source::last_push_at(noop_db, &installation, streams).await?;
                last_push_at = pushed.map(calendar::iso);
                for day in days(from_day, to_day) {
                    let edited = if noop_metric.reads_sleep() {
                        recovery::sleep_edited(noop_db, &installation, day).await?
                    } else {
                        false
                    };
                    let resolved =
                        recovery::daily_metrics(noop_db, &installation, day, edited).await?;
                    let sourced = noop_metric.select(&resolved);
                    let rows = [Rows::DailyMetrics(day), Rows::SleepSessions(day)];
                    let relevant = if noop_metric.reads_sleep() {
                        &rows[..]
                    } else {
                        &rows[..1]
                    };
                    let mut covered = true;
                    for &row in relevant {
                        covered &= noop_source::coverage(noop_db, &installation, row, None).await?
                            == Coverage::Covered;
                    }
                    let coverage = if covered {
                        Coverage::Covered
                    } else {
                        Coverage::Unknown
                    };
                    let freshness =
                        noop_source::freshness(noop_db, &installation, pushed, day, relevant)
                            .await?;
                    daily_points.push(DailyPoint {
                        day: day.to_string(),
                        value: sourced.value,
                        unit,
                        source: sourced.source.map(Namespace::device_id),
                        freshness: Some(freshness),
                        coverage: Some(coverage),
                    });
                }
            } else {
                for day in days(from_day, to_day) {
                    daily_points.push(DailyPoint {
                        day: day.to_string(),
                        value: None,
                        unit,
                        source: None,
                        freshness: Some(Freshness::Unknown),
                        coverage: Some(Coverage::Unknown),
                    });
                }
            }
        }
    }

    let mut weekly_summaries = Vec::new();
    let mut week: Option<WeekAccumulator> = None;
    for (day, point) in days(from_day, to_day).zip(&daily_points) {
        let (week_start, week_end) = day.iso_week_bounds().ok_or_else(|| {
            AppError::Validation("Metric trend dates are outside supported ISO weeks".to_owned())
        })?;
        if week
            .as_ref()
            .is_some_and(|current| current.summary.week_start != week_start.to_string())
            && let Some(finished) = week.take()
        {
            weekly_summaries.push(finished.finish());
        }
        let current =
            week.get_or_insert_with(|| WeekAccumulator::new(day, week_start, week_end, unit));
        current.add(day, point.value);
    }
    if let Some(week) = week {
        weekly_summaries.push(week.finish());
    }

    Ok(MetricTrend {
        metric: metric.name(),
        from_day: from_day.to_string(),
        to_day: to_day.to_string(),
        time_zone: TIME_ZONE_NAME,
        unit,
        installation_id,
        last_push_at,
        daily_points,
        weekly_summaries,
    })
}

fn days(from: CalendarDay, to: CalendarDay) -> impl Iterator<Item = CalendarDay> {
    std::iter::successors(Some(from), move |day| (*day < to).then(|| day.next()))
}
