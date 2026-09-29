//! Training Context: a compact view of a training block on one Europe/Copenhagen date axis.
//!
//! Every day from `from_day` through `to_day` has an entry, even with nothing logged or synced. An
//! entry summarizes the Workouts logged that date and the NOOP Workouts starting that local day,
//! as two lists that are never paired, beside daily Body Weight and the Recovery Day's sleep
//! duration, resting heart rate and HRV. Exact Sets and sleep sessions stay in the focused reads.

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

/// Logged Exercise entries a Training Context may hold (a Workout without Exercises counts as
/// one). A larger block is rejected rather than truncated.
pub const MAX_LOGGED_ENTRIES: usize = 1000;
/// Bytes the serialized JSON response may take. Row caps bound the entries but not the free-text
/// Workout, Exercise and NOOP sport names in them. A larger body is rejected rather than truncated.
pub const MAX_RESPONSE_BYTES: usize = 512 * 1024;

#[derive(Debug, Serialize)]
pub struct TrainingContext {
    pub from_day: String,
    pub to_day: String,
    pub time_zone: &'static str,
    pub noop: ContextSync,
    /// One entry per calendar day, oldest first.
    pub days: Vec<ContextDay>,
}

/// The NOOP installation and the watermarks behind each day's freshness.
#[derive(Debug, Serialize)]
pub struct ContextSync {
    pub installation_id: Option<String>,
    pub imported_device_id: &'static str,
    pub computed_device_id: &'static str,
    pub last_push_at: PushWatermarks,
}

#[derive(Debug, Serialize)]
pub struct PushWatermarks {
    /// `dailyMetric` and `sleepSession` in both strap namespaces, as for a Recovery Day.
    pub recovery: Option<String>,
    /// `workout` in every workout namespace, as for Workouts on a Training Day.
    pub workouts: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ContextDay {
    pub day: String,
    /// Workouts logged in `OpenHome` with this date, in the order they were created.
    pub workouts: Vec<WorkoutSummary>,
    /// NOOP Workouts whose start falls inside the local day, by start; never paired with a logged
    /// Workout.
    pub noop_workouts: Vec<NoopWorkoutSummary>,
    /// The daily Body Weight record; `source` is `body_weight` when one exists.
    pub body_weight: Measurement<f64>,
    /// The Recovery Day's total sleep for the Sleep Night waking on this day.
    pub sleep_duration: Measurement<f64>,
    pub resting_hr: Measurement<f64>,
    /// Nightly heart-rate variability as RMSSD.
    pub hrv_rmssd: Measurement<f64>,
    pub noop: ContextDaySync,
}

#[derive(Debug, Serialize)]
pub struct WorkoutSummary {
    pub id: i64,
    pub name: Option<String>,
    /// In logged order; repeated entries of one Exercise stay separate.
    pub exercises: Vec<ExerciseSummary>,
}

/// One Exercise entry's Sets, reduced to counts and bests; exact Sets stay in the focused reads.
#[derive(Debug, Serialize)]
pub struct ExerciseSummary {
    pub exercise: String,
    pub sets: i64,
    pub best_reps: Option<i64>,
    /// Heaviest weight added to the body; null when every Set was bodyweight.
    pub best_added_weight_kg: Option<f64>,
    /// Longest Timed Hold.
    pub best_hold_duration_s: Option<i64>,
    pub best_rpe: Option<i64>,
    /// Volume: reps (or, for a weighted Timed Hold, seconds) times Added Weight; bodyweight Sets
    /// add zero. Null without Sets.
    pub volume_kg: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct NoopWorkoutSummary {
    pub start: String,
    pub end: String,
    pub sport: String,
    pub origin: WorkoutOrigin,
    /// NOOP's recorded duration.
    pub duration_min: Option<f64>,
    pub avg_hr_bpm: Option<f64>,
    /// NOOP's derived 0-100 workout strain; a model output, not a measurement.
    pub strain_score: Option<f64>,
    /// The NOOP device namespace of the stored row.
    pub source: String,
}

/// Each NOOP read behind a day's values, with the freshness and coverage of that read's rows.
#[derive(Debug, Serialize)]
pub struct ContextDaySync {
    /// The Recovery Day's rows, behind `sleep_duration`, `resting_hr` and `hrv_rmssd`.
    pub recovery: DaySync<NoopCoverage>,
    /// The Training Day's NOOP workout rows, behind `noop_workouts`.
    pub workouts: DaySync<WorkoutCoverage>,
}

/// The route validates the 1 through 90 day bound before calling this function.
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
            let edited = recovery::sleep_edited(noop_db, installation, day).await?;
            let daily = recovery::daily_metrics(noop_db, installation, day, edited).await?;
            entry.sleep_duration = Measurement::new(daily.total_sleep_min, MINUTES);
            entry.resting_hr = Measurement::new(daily.resting_hr, BEATS_PER_MINUTE);
            entry.hrv_rmssd = Measurement::new(daily.avg_hrv, MILLISECONDS);
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

/// Whether `value`'s compact JSON, as the response writes it, fits in `limit` bytes. Serialization
/// stops at the first byte past the limit.
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

/// Logged Workouts by date, each Exercise entry summarized in one grouped query.
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

/// A value the receiver has not established: no installation has pushed yet.
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
        source: row.device_id.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_budget_includes_its_limit() {
        // `"abcd"` is six bytes of JSON.
        assert!(serializes_within(&"abcd", 6).unwrap());
        assert!(!serializes_within(&"abcd", 5).unwrap());
        assert!(!serializes_within(&vec!["abcd"; 1000], 6).unwrap());
    }
}
