//! Combined Progress Lines: a bounded batch read that indexes several Exercises' observed progress
//! to a shared scale, so one chart can compare movements logged in different units.
//!
//! Each Exercise gets one type-appropriate metric chosen from its Sets in the period:
//! - bodyweight repetitions: the best reps of any unweighted reps Set on a day;
//! - Timed Hold seconds: the longest unweighted hold on a day.
//!
//! The line is indexed to 100 at its first positive sample in the period. Only observed days have
//! samples, so gaps, plateaus and declines stay visible; nothing is interpolated or clamped.
//! Exercises whose Sets carry Added Weight, or mix reps with holds, are reported as unsupported
//! instead of being forced onto a misleading line. The per-exercise Progress Series endpoint is
//! unchanged.

use std::collections::HashMap;

use serde::Serialize;
use sqlx::SqlitePool;

use crate::error::{AppError, Result};
use crate::services::training::CalendarDay;

/// Most Exercises one read returns; also the default selection's size.
pub const MAX_EXERCISES: usize = 24;

/// Longest period, inclusive, so a year of training fits with its first day.
pub const MAX_DAYS: i64 = 366;

/// An inclusive period of 1 through [`MAX_DAYS`] calendar days.
#[derive(Debug, Clone, Copy)]
pub struct Period {
    from: CalendarDay,
    to: CalendarDay,
}

impl Period {
    /// Parses canonical `YYYY-MM-DD` bounds, rejecting reversed and overlong periods.
    pub fn parse(from: &str, to: &str) -> Result<Self> {
        let day = |text: &str| {
            CalendarDay::parse(text).ok_or_else(|| {
                AppError::Validation(format!("Invalid day '{text}': must be written YYYY-MM-DD"))
            })
        };
        let (from, to) = (day(from)?, day(to)?);
        if !(0..MAX_DAYS).contains(&from.days_until(to)) {
            return Err(AppError::Validation(format!(
                "Progress must cover 1 to {MAX_DAYS} calendar days, with from <= to"
            )));
        }
        Ok(Self { from, to })
    }
}

/// Which Exercises to read: explicitly chosen ones, or the most recently logged in the period.
#[derive(Debug)]
pub enum Selection {
    Recent,
    Chosen(Vec<i64>),
}

impl Selection {
    /// Parses a comma-separated list of 1 through [`MAX_EXERCISES`] distinct Exercise ids;
    /// `None` selects the most recently logged Exercises.
    pub fn parse(ids: Option<&str>) -> Result<Self> {
        let Some(ids) = ids else {
            return Ok(Self::Recent);
        };
        let invalid = || {
            AppError::Validation(format!(
                "exercise_ids must list 1 to {MAX_EXERCISES} distinct Exercise ids separated by commas"
            ))
        };
        let mut parsed = Vec::new();
        for id in ids.split(',') {
            let id = id.trim().parse::<i64>().map_err(|_| invalid())?;
            if parsed.contains(&id) {
                return Err(invalid());
            }
            parsed.push(id);
        }
        if parsed.len() > MAX_EXERCISES {
            return Err(invalid());
        }
        Ok(Self::Chosen(parsed))
    }
}

#[derive(Debug, Serialize)]
pub struct CombinedProgress {
    pub from: String,
    pub to: String,
    /// Distinct Exercises with Sets in the period, including any beyond the default selection.
    pub logged_exercise_count: usize,
    pub exercises: Vec<ProgressLine>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExerciseRef {
    pub id: i64,
    pub name: String,
    pub category: String,
}

#[derive(Debug, Serialize)]
pub struct ProgressLine {
    pub exercise: ExerciseRef,
    #[serde(flatten)]
    pub progress: LineProgress,
}

#[derive(Debug, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum LineProgress {
    Charted {
        metric: Metric,
        unit: &'static str,
        baseline: Sample,
        samples: Vec<IndexedSample>,
    },
    Unsupported {
        reason: Unsupported,
    },
    /// Logged before or after the period, but not in it.
    NotLoggedInRange,
    NeverLogged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Metric {
    Repetitions,
    HoldSeconds,
}

impl Metric {
    const fn unit(self) -> &'static str {
        match self {
            Self::Repetitions => "reps",
            Self::HoldSeconds => "s",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Unsupported {
    /// At least one Set carries Added Weight, which a reps or seconds line cannot represent.
    AddedWeight,
    /// Bodyweight reps and Timed Holds were both logged, so no single metric fits.
    MixedSetKinds,
    /// Every observed value is zero, leaving no baseline to index against.
    NoPositiveValue,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Sample {
    pub date: String,
    pub value: i64,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct IndexedSample {
    pub date: String,
    pub value: i64,
    /// `value` relative to the baseline, where the baseline is 100.
    pub index: f64,
}

/// One Exercise's Sets on one Workout date, reduced to what metric selection needs.
#[derive(Debug, Clone)]
struct LoggedDay {
    exercise_id: i64,
    date: String,
    weighted_sets: i64,
    rep_sets: i64,
    hold_sets: i64,
    best_reps: Option<i64>,
    best_hold_s: Option<i64>,
}

/// Reads the selected Exercises' Progress Lines for the period.
pub async fn combined_progress(
    db: &SqlitePool,
    period: Period,
    selection: Selection,
) -> Result<CombinedProgress> {
    let from = period.from.to_string();
    let to = period.to.to_string();

    // Every Exercise logged in the period, most recent first: the count and default selection.
    let logged = sqlx::query_as!(
        ExerciseRef,
        r#"
        SELECT e.id AS "id!: i64", e.name, e.category
        FROM exercises e
        JOIN workout_exercises we ON we.exercise_id = e.id
        JOIN sets s ON s.workout_exercise_id = we.id
        JOIN workouts w ON w.id = we.workout_id
        WHERE w.date BETWEEN $1 AND $2
        GROUP BY e.id
        ORDER BY MAX(w.date) DESC, e.name COLLATE NOCASE, e.id
        "#,
        from,
        to
    )
    .fetch_all(db)
    .await
    .map_err(anyhow::Error::from)?;
    let logged_exercise_count = logged.len();

    let (exercises, ever_logged) = match selection {
        Selection::Recent => {
            let recent: Vec<ExerciseRef> = logged.into_iter().take(MAX_EXERCISES).collect();
            let ever_logged = recent.iter().map(|exercise| exercise.id).collect();
            (recent, ever_logged)
        }
        Selection::Chosen(ids) => chosen_exercises(db, &ids).await?,
    };

    let ids = serde_json::to_string(&exercises.iter().map(|e| e.id).collect::<Vec<_>>())
        .map_err(anyhow::Error::from)?;
    let rows = sqlx::query_as!(
        LoggedDay,
        r#"
        SELECT we.exercise_id AS "exercise_id!: i64",
               CAST(w.date AS TEXT) AS "date!: String",
               CAST(SUM(COALESCE(s.weight_kg, 0) <> 0) AS INTEGER) AS "weighted_sets!: i64",
               CAST(SUM(COALESCE(s.weight_kg, 0) = 0 AND s.reps IS NOT NULL) AS INTEGER) AS "rep_sets!: i64",
               CAST(SUM(COALESCE(s.weight_kg, 0) = 0 AND s.reps IS NULL) AS INTEGER) AS "hold_sets!: i64",
               CAST(MAX(CASE WHEN COALESCE(s.weight_kg, 0) = 0 THEN s.reps END) AS INTEGER) AS "best_reps?: i64",
               CAST(MAX(CASE WHEN COALESCE(s.weight_kg, 0) = 0 AND s.reps IS NULL
                             THEN s.duration_seconds END) AS INTEGER) AS "best_hold_s?: i64"
        FROM sets s
        JOIN workout_exercises we ON we.id = s.workout_exercise_id
        JOIN workouts w ON w.id = we.workout_id
        WHERE we.exercise_id IN (SELECT value FROM json_each($1))
          AND w.date BETWEEN $2 AND $3
        GROUP BY we.exercise_id, w.date
        ORDER BY we.exercise_id, w.date
        "#,
        ids,
        from,
        to
    )
    .fetch_all(db)
    .await
    .map_err(anyhow::Error::from)?;

    let mut days: HashMap<i64, Vec<LoggedDay>> = HashMap::new();
    for row in rows {
        days.entry(row.exercise_id).or_default().push(row);
    }

    let exercises = exercises
        .into_iter()
        .map(|exercise| {
            let progress = match days.get(&exercise.id) {
                Some(days) => line(days),
                None if ever_logged.contains(&exercise.id) => LineProgress::NotLoggedInRange,
                None => LineProgress::NeverLogged,
            };
            ProgressLine { exercise, progress }
        })
        .collect();

    Ok(CombinedProgress {
        from,
        to,
        logged_exercise_count,
        exercises,
    })
}

/// The chosen Exercises in request order, plus the ids with any logged Set at all.
async fn chosen_exercises(db: &SqlitePool, ids: &[i64]) -> Result<(Vec<ExerciseRef>, Vec<i64>)> {
    let json_ids = serde_json::to_string(ids).map_err(anyhow::Error::from)?;
    let rows = sqlx::query!(
        r#"
        SELECT e.id AS "id!: i64", e.name, e.category,
               EXISTS (SELECT 1 FROM workout_exercises we
                       JOIN sets s ON s.workout_exercise_id = we.id
                       WHERE we.exercise_id = e.id) AS "ever_logged!: bool"
        FROM exercises e
        WHERE e.id IN (SELECT value FROM json_each($1))
        "#,
        json_ids
    )
    .fetch_all(db)
    .await
    .map_err(anyhow::Error::from)?;

    let mut exercises = Vec::with_capacity(ids.len());
    let mut ever_logged = Vec::new();
    for id in ids {
        let row = rows
            .iter()
            .find(|row| row.id == *id)
            .ok_or_else(|| AppError::NotFound(format!("Exercise with id {id} not found")))?;
        if row.ever_logged {
            ever_logged.push(row.id);
        }
        exercises.push(ExerciseRef {
            id: row.id,
            name: row.name.clone(),
            category: row.category.clone(),
        });
    }
    Ok((exercises, ever_logged))
}

/// Chooses the metric from the period's Set kinds and indexes each observed day to the baseline.
fn line(days: &[LoggedDay]) -> LineProgress {
    let total = |count: fn(&LoggedDay) -> i64| days.iter().map(count).sum::<i64>();
    let metric = match (
        total(|day| day.weighted_sets),
        total(|day| day.rep_sets),
        total(|day| day.hold_sets),
    ) {
        (weighted, _, _) if weighted > 0 => {
            return LineProgress::Unsupported {
                reason: Unsupported::AddedWeight,
            };
        }
        (_, reps, holds) if reps > 0 && holds > 0 => {
            return LineProgress::Unsupported {
                reason: Unsupported::MixedSetKinds,
            };
        }
        (_, reps, _) if reps > 0 => Metric::Repetitions,
        _ => Metric::HoldSeconds,
    };

    let observed: Vec<Sample> = days
        .iter()
        .filter_map(|day| {
            let value = match metric {
                Metric::Repetitions => day.best_reps,
                Metric::HoldSeconds => day.best_hold_s,
            }?;
            Some(Sample {
                date: day.date.clone(),
                value,
            })
        })
        .collect();
    let Some(baseline) = observed.iter().find(|sample| sample.value > 0).cloned() else {
        return LineProgress::Unsupported {
            reason: Unsupported::NoPositiveValue,
        };
    };

    #[allow(clippy::cast_precision_loss)] // Logged reps and seconds are far below 2^52.
    let samples = observed
        .into_iter()
        .map(|sample| IndexedSample {
            index: sample.value as f64 / baseline.value as f64 * 100.0,
            date: sample.date,
            value: sample.value,
        })
        .collect();

    LineProgress::Charted {
        metric,
        unit: metric.unit(),
        baseline,
        samples,
    }
}
