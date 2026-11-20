use std::collections::HashMap;
use std::io;

use serde::Serialize;
use sqlx::SqlitePool;

use crate::error::{AppError, Result};

use super::calendar::{self, CalendarDay, TIME_ZONE_NAME, days};
use super::noop_merge::{self, WorkoutOrigin, WorkoutRow};
use super::noop_source::{
    self, COMPUTED_DEVICE_ID, Coverage, DaySync, Freshness, IMPORTED_DEVICE_ID,
};
use super::recovery::{
    self, BEATS_PER_MINUTE, MILLISECONDS, MINUTES, Measurement, NoopCoverage, RECOVERY_STREAMS,
};
use super::trend::{self, BODY_WEIGHT_SOURCE, KILOGRAMS};
use super::workouts::{self, WORKOUT_STREAMS, WorkoutCoverage};

pub const MAX_LOGGED_ENTRIES: usize = 1000;
pub const MAX_RESPONSE_BYTES: usize = 512 * 1024;

#[derive(Debug, Serialize)]
pub struct TrainingContext {
    pub from_day: String,
    pub to_day: String,
    pub time_zone: &'static str,
    pub noop: ContextSync,
    pub days: Vec<ContextDay>,
}

#[derive(Debug, Serialize)]
pub struct ContextSync {
    pub installation_id: Option<String>,
    pub imported_device_id: &'static str,
    pub computed_device_id: &'static str,
    pub last_push_at: PushWatermarks,
}

#[derive(Debug, Serialize)]
pub struct PushWatermarks {
    pub recovery: Option<String>,
    pub workouts: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ContextDay {
    pub day: String,
    pub workouts: Vec<WorkoutSummary>,
    pub noop_workouts: Vec<NoopWorkoutSummary>,
    pub body_weight: Measurement<f64>,
    pub sleep_duration: Measurement<f64>,
    pub resting_hr: Measurement<f64>,
    pub hrv_rmssd: Measurement<f64>,
    pub noop: ContextDaySync,
}

#[derive(Debug, Serialize)]
pub struct WorkoutSummary {
    pub id: i64,
    pub name: Option<String>,
    pub exercises: Vec<ExerciseSummary>,
}

#[derive(Debug, Serialize)]
pub struct ExerciseSummary {
    pub exercise: String,
    pub sets: i64,
    pub best_reps: Option<i64>,
    pub best_added_weight_kg: Option<f64>,
    pub best_hold_duration_s: Option<i64>,
    pub best_rpe: Option<i64>,
    pub volume_kg: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct NoopWorkoutSummary {
    pub start: String,
    pub end: String,
    pub sport: String,
    pub origin: WorkoutOrigin,
    pub duration_min: Option<f64>,
    pub avg_hr_bpm: Option<f64>,
    pub strain_score: Option<f64>,
    pub steps: Option<i64>,
    pub source: String,
}

#[derive(Debug, Serialize)]
pub struct ContextDaySync {
    pub recovery: DaySync<NoopCoverage>,
    pub workouts: DaySync<WorkoutCoverage>,
}

pub async fn training_context(
    db: &SqlitePool,
    noop_db: &SqlitePool,
    from_day: CalendarDay,
    to_day: CalendarDay,
) -> Result<TrainingContext> {
    let mut logged = logged_workouts(db, from_day, to_day).await?;
    let mut body_weights = trend::body_weights(db, from_day, to_day).await?;
    let installation = noop_source::active_installation(noop_db).await?;
    let (recovery_pushed, workouts_pushed) = match &installation {
        Some(installation) => (
            noop_source::last_push_at(noop_db, installation, &RECOVERY_STREAMS).await?,
            noop_source::last_push_at(noop_db, installation, &WORKOUT_STREAMS).await?,
        ),
        None => (None, None),
    };

    let mut context_days = Vec::new();
    for day in days(from_day, to_day) {
        let key = day.to_string();
        let body_weight = body_weights.remove(&key);
        let mut entry = ContextDay {
            day: key.clone(),
            workouts: logged.remove(&key).unwrap_or_default(),
            noop_workouts: Vec::new(),
            body_weight: Measurement {
                value: body_weight,
                unit: KILOGRAMS,
                source: body_weight.map(|_| BODY_WEIGHT_SOURCE),
            },
            sleep_duration: missing(MINUTES),
            resting_hr: missing(BEATS_PER_MINUTE),
            hrv_rmssd: missing(MILLISECONDS),
            noop: ContextDaySync {
                recovery: DaySync {
                    freshness: Freshness::Unknown,
                    coverage: NoopCoverage {
                        daily_metrics: Coverage::Unknown,
                        sleep_sessions: Coverage::Unknown,
                    },
                },
                workouts: DaySync {
                    freshness: Freshness::Unknown,
                    coverage: WorkoutCoverage {
                        workouts: Coverage::Unknown,
                    },
                },
            },
        };
        if let Some(installation) = &installation {
            let measurements = recovery::training_measurements(noop_db, installation, day).await?;
            entry.sleep_duration = measurements.sleep_duration;
            entry.resting_hr = measurements.resting_hr;
            entry.hrv_rmssd = measurements.hrv_rmssd;
            entry.noop_workouts = noop_merge::merge_workouts(
                workouts::noop_workout_rows(noop_db, installation, day).await?,
            )
            .iter()
            .map(noop_workout)
            .collect::<Result<_>>()?;

            entry.noop = ContextDaySync {
                recovery: recovery::day_sync(noop_db, installation, recovery_pushed, day).await?,
                workouts: workouts::day_sync(noop_db, installation, workouts_pushed, day).await?,
            };
        }
        context_days.push(entry);
    }

    let context = TrainingContext {
        from_day: from_day.to_string(),
        to_day: to_day.to_string(),
        time_zone: TIME_ZONE_NAME,
        noop: ContextSync {
            installation_id: installation.map(|installation| installation.source_id),
            imported_device_id: IMPORTED_DEVICE_ID,
            computed_device_id: COMPUTED_DEVICE_ID,
            last_push_at: PushWatermarks {
                recovery: recovery_pushed.map(calendar::iso),
                workouts: workouts_pushed.map(calendar::iso),
            },
        },
        days: context_days,
    };
    if !serializes_within(&context, MAX_RESPONSE_BYTES)? {
        return Err(AppError::Unprocessable(format!(
            "The Training Context from {from_day} to {to_day} is larger than \
             {MAX_RESPONSE_BYTES} bytes; refusing to return a truncated block"
        )));
    }
    Ok(context)
}

fn serializes_within(value: &impl Serialize, limit: usize) -> Result<bool> {
    struct Budget(usize);

    impl io::Write for Budget {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0 = self
                .0
                .checked_sub(buf.len())
                .ok_or_else(|| io::Error::other("over budget"))?;
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    match serde_json::to_writer(Budget(limit), value) {
        Ok(()) => Ok(true),
        Err(err) if err.is_io() => Ok(false),
        Err(err) => Err(anyhow::Error::from(err).into()),
    }
}

async fn logged_workouts(
    db: &SqlitePool,
    from_day: CalendarDay,
    to_day: CalendarDay,
) -> Result<HashMap<String, Vec<WorkoutSummary>>> {
    let from = from_day.to_string();
    let to = to_day.to_string();
    let limit = MAX_LOGGED_ENTRIES as i64 + 1;
    let rows = sqlx::query!(
        r#"
        SELECT w.date                  AS "date!: String",
               w.id                    AS "workout_id!: i64",
               w.name                  AS workout_name,
               e.name                  AS "exercise?: String",
               COUNT(s.id)             AS "sets!: i64",
               MAX(s.reps)             AS "best_reps?: i64",
               MAX(s.weight_kg)        AS "best_added_weight_kg?: f64",
               MAX(s.duration_seconds) AS "best_hold_duration_s?: i64",
               MAX(s.rpe)              AS "best_rpe?: i64",
               CAST(SUM(COALESCE(s.reps, s.duration_seconds) * COALESCE(s.weight_kg, 0.0))
                    AS REAL)           AS "volume_kg?: f64"
        FROM workouts w
        LEFT JOIN workout_exercises we ON we.workout_id = w.id
        LEFT JOIN exercises e ON e.id = we.exercise_id
        LEFT JOIN sets s ON s.workout_exercise_id = we.id
        WHERE w.date BETWEEN $1 AND $2
        GROUP BY w.id, we.id
        ORDER BY w.date, w.id, we.order_index, we.id
        LIMIT $3
        "#,
        from,
        to,
        limit
    )
    .fetch_all(db)
    .await
    .map_err(anyhow::Error::from)?;
    if rows.len() > MAX_LOGGED_ENTRIES {
        return Err(AppError::Unprocessable(format!(
            "The Workouts logged from {from_day} to {to_day} have more than {MAX_LOGGED_ENTRIES} \
             Exercise entries; refusing to return a truncated Training Context"
        )));
    }

    let mut by_day: HashMap<String, Vec<WorkoutSummary>> = HashMap::new();
    for row in rows {
        let workouts = by_day.entry(row.date).or_default();
        if workouts
            .last()
            .is_none_or(|workout| workout.id != row.workout_id)
        {
            workouts.push(WorkoutSummary {
                id: row.workout_id,
                name: row.workout_name,
                exercises: Vec::new(),
            });
        }
        let Some(exercise) = row.exercise else {
            continue;
        };
        workouts
            .last_mut()
            .expect("pushed above")
            .exercises
            .push(ExerciseSummary {
                exercise,
                sets: row.sets,
                best_reps: row.best_reps,
                best_added_weight_kg: row.best_added_weight_kg,
                best_hold_duration_s: row.best_hold_duration_s,
                best_rpe: row.best_rpe,
                volume_kg: row.volume_kg,
            });
    }
    Ok(by_day)
}

const fn missing(unit: &'static str) -> Measurement<f64> {
    Measurement {
        value: None,
        unit,
        source: None,
    }
}

fn noop_workout(row: &WorkoutRow) -> Result<NoopWorkoutSummary> {
    Ok(NoopWorkoutSummary {
        start: calendar::iso_from_unix(row.start_ts)?,
        end: calendar::iso_from_unix(row.end_ts)?,
        sport: row.sport.clone(),
        origin: row.origin,
        duration_min: row.duration_s.map(|seconds| seconds / 60.0),
        avg_hr_bpm: row.avg_hr,
        strain_score: row.strain,
        steps: row.steps,
        source: row.device_id.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_budget_includes_its_limit() {
        assert!(serializes_within(&"abcd", 6).unwrap());
        assert!(!serializes_within(&"abcd", 5).unwrap());
        assert!(!serializes_within(&vec!["abcd"; 1000], 6).unwrap());
    }
}
