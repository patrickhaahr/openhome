//! Workouts on a Training Day: the Workouts logged in OpenHome for one Copenhagen date beside the
//! NOOP Workouts that started on it.
//!
//! The two stay separate lists. OpenHome Workouts are date-only and NOOP Workouts are timed, so
//! nothing pairs a NOOP Workout with a logged one. The `noop` block says whether an empty or short
//! NOOP list is a recorded absence or a sync that has not arrived; even a covered empty list only
//! means NOOP recorded no workout, not that the user did not train.

use serde::Serialize;
use sqlx::SqlitePool;

use crate::error::{AppError, Result};

use super::calendar::{self, CalendarDay, TIME_ZONE_NAME};
use super::noop_merge::{self, WorkoutOrigin, WorkoutRow};
use super::noop_source::{
    self, COMPUTED_DEVICE_ID, Coverage, IMPORTED_DEVICE_ID, Installation, NoopSync, Rows, Stream,
};

/// Logged Set rows one Training Day may hold (an Exercise without Sets, or a Workout without
/// Exercises, counts as one). A larger day is rejected rather than truncated.
pub const MAX_LOGGED_ROWS: usize = 500;
/// NOOP workout rows, across all namespaces, one Training Day may hold.
pub const MAX_NOOP_WORKOUTS: usize = 24;

#[derive(Debug, Serialize)]
pub struct WorkoutsOnDay {
    pub day: String,
    pub time_zone: &'static str,
    /// Local midnight at the start of the day.
    pub day_start: String,
    /// Local midnight at the end of the day (exclusive).
    pub day_end: String,
    /// Workouts logged in OpenHome with this date, in the order they were created.
    pub workouts: Vec<LoggedWorkout>,
    /// NOOP Workouts whose start falls inside the day, by start.
    pub noop_workouts: Vec<NoopWorkout>,
    pub noop: NoopSync<WorkoutCoverage>,
}

#[derive(Debug, Serialize)]
pub struct LoggedWorkout {
    pub id: i64,
    pub name: Option<String>,
    pub notes: Option<String>,
    /// In logged order.
    pub exercises: Vec<LoggedExercise>,
}

#[derive(Debug, Serialize)]
pub struct LoggedExercise {
    pub exercise_id: i64,
    pub exercise: String,
    pub category: String,
    pub notes: Option<String>,
    /// By set number.
    pub sets: Vec<LoggedSet>,
}

#[derive(Debug, Serialize)]
pub struct LoggedSet {
    pub set_number: i64,
    pub reps: Option<i64>,
    /// Weight added to the body; null for a bodyweight Set.
    pub added_weight_kg: Option<f64>,
    /// Timed Hold duration.
    pub hold_duration_s: Option<i64>,
    pub rpe: Option<i64>,
    pub notes: Option<String>,
}

/// One NOOP Workout as NOOP stored it. Heart rate is the stored average and peak.
#[derive(Debug, Serialize)]
pub struct NoopWorkout {
    pub start: String,
    pub end: String,
    pub sport: String,
    pub origin: WorkoutOrigin,
    /// NOOP's recorded duration.
    pub duration_min: Option<f64>,
    pub avg_hr_bpm: Option<f64>,
    pub max_hr_bpm: Option<f64>,
    /// NOOP's derived 0-100 workout strain; a model output, not a measurement.
    pub strain_score: Option<f64>,
    pub energy_kcal: Option<f64>,
    pub distance_m: Option<f64>,
    pub steps: Option<i64>,
    pub notes: Option<String>,
    /// The NOOP device namespace of the stored row.
    pub source: String,
}

#[derive(Debug, Serialize)]
pub struct WorkoutCoverage {
    pub workouts: Coverage,
}

pub async fn workouts_on_day(
    db: &SqlitePool,
    noop_db: &SqlitePool,
    day: CalendarDay,
) -> Result<WorkoutsOnDay> {
    let workouts = logged_workouts(db, day).await?;
    let (noop_workouts, noop) = match noop_source::active_installation(noop_db).await? {
        None => (
            Vec::new(),
            NoopSync::unsynced(WorkoutCoverage {
                workouts: Coverage::Unknown,
            }),
        ),
        Some(installation) => {
            let rows = [Rows::Workouts(day)];
            let last_push_at =
                noop_source::last_push_at(noop_db, &installation, &[Stream::Workout]).await?;
            let coverage = WorkoutCoverage {
                workouts: noop_source::coverage(noop_db, &installation, rows[0], None).await?,
            };
            let freshness =
                noop_source::freshness(noop_db, &installation, last_push_at, day, &rows).await?;
            let noop_workouts =
                noop_merge::merge_workouts(noop_workout_rows(noop_db, &installation, day).await?)
                    .iter()
                    .map(noop_workout)
                    .collect::<Result<_>>()?;
            let sync = NoopSync::synced(installation, last_push_at, freshness, coverage);
            (noop_workouts, sync)
        }
    };
    Ok(WorkoutsOnDay {
        day: day.to_string(),
        time_zone: TIME_ZONE_NAME,
        day_start: calendar::iso(day.start()),
        day_end: calendar::iso(day.end()),
        workouts,
        noop_workouts,
        noop,
    })
}

/// The day's logged Workouts with their Exercises and Sets, from one ordered join.
async fn logged_workouts(db: &SqlitePool, day: CalendarDay) -> Result<Vec<LoggedWorkout>> {
    let date = day.to_string();
    let limit = MAX_LOGGED_ROWS as i64 + 1;
    let rows = sqlx::query!(
        r#"
        SELECT w.id                AS "workout_id!: i64",
               w.name              AS workout_name,
               w.notes             AS workout_notes,
               we.id               AS "entry_id?: i64",
               we.notes            AS entry_notes,
               e.id                AS "exercise_id?: i64",
               e.name              AS "exercise_name?: String",
               e.category          AS "exercise_category?: String",
               s.id                AS "set_id?: i64",
               s.set_number        AS "set_number?: i64",
               s.reps,
               s.weight_kg,
               s.duration_seconds,
               s.rpe,
               s.notes             AS set_notes
        FROM workouts w
        LEFT JOIN workout_exercises we ON we.workout_id = w.id
        LEFT JOIN exercises e ON e.id = we.exercise_id
        LEFT JOIN sets s ON s.workout_exercise_id = we.id
        WHERE w.date = $1
        ORDER BY w.id, we.order_index, we.id, s.set_number, s.id
        LIMIT $2
        "#,
        date,
        limit
    )
    .fetch_all(db)
    .await
    .map_err(anyhow::Error::from)?;
    if rows.len() > MAX_LOGGED_ROWS {
        return Err(AppError::Unprocessable(format!(
            "The Workouts logged on {day} have more than {MAX_LOGGED_ROWS} Set rows; refusing to \
             return a truncated day"
        )));
    }

    let mut workouts: Vec<LoggedWorkout> = Vec::new();
    let mut last_entry_id = None;
    for row in rows {
        if workouts
            .last()
            .is_none_or(|workout| workout.id != row.workout_id)
        {
            workouts.push(LoggedWorkout {
                id: row.workout_id,
                name: row.workout_name,
                notes: row.workout_notes,
                exercises: Vec::new(),
            });
        }
        let workout = workouts.last_mut().expect("pushed above");
        let (Some(entry_id), Some(exercise_id), Some(exercise), Some(category)) = (
            row.entry_id,
            row.exercise_id,
            row.exercise_name,
            row.exercise_category,
        ) else {
            continue;
        };
        if last_entry_id != Some(entry_id) {
            last_entry_id = Some(entry_id);
            workout.exercises.push(LoggedExercise {
                exercise_id,
                exercise,
                category,
                notes: row.entry_notes,
                sets: Vec::new(),
            });
        }
        let (Some(_), Some(set_number)) = (row.set_id, row.set_number) else {
            continue;
        };
        workout
            .exercises
            .last_mut()
            .expect("pushed above")
            .sets
            .push(LoggedSet {
                set_number,
                reps: row.reps,
                added_weight_kg: row.weight_kg,
                hold_duration_s: row.duration_seconds,
                rpe: row.rpe,
                notes: row.set_notes,
            });
    }
    Ok(workouts)
}

/// The installation's workouts that start on `day`, strap namespaces first, before selection.
async fn noop_workout_rows(
    pool: &SqlitePool,
    installation: &Installation,
    day: CalendarDay,
) -> Result<Vec<WorkoutRow>> {
    #[derive(sqlx::FromRow)]
    struct Record {
        device_id: String,
        start_ts: i64,
        end_ts: i64,
        sport: String,
        source: String,
        duration_s: Option<f64>,
        energy_kcal: Option<f64>,
        avg_hr: Option<f64>,
        max_hr: Option<f64>,
        strain: Option<f64>,
        distance_m: Option<f64>,
        steps: Option<i64>,
        notes: Option<String>,
        has_zones: bool,
    }

    let records: Vec<Record> = sqlx::query_as(
        "SELECT device_id, start_ts, end_ts, sport, source, duration_s, energy_kcal, avg_hr, \
                max_hr, strain, distance_m, steps, notes, \
                COALESCE(zones_json, '') != '' AS has_zones \
         FROM workout \
         WHERE source_id = ? AND start_ts >= ? AND start_ts < ? \
         ORDER BY CASE device_id WHEN ? THEN 0 WHEN ? THEN 1 ELSE 2 END, \
                  device_id, start_ts, sport LIMIT ?",
    )
    .bind(&installation.source_id)
    .bind(day.start().timestamp())
    .bind(day.end().timestamp())
    .bind(IMPORTED_DEVICE_ID)
    .bind(COMPUTED_DEVICE_ID)
    .bind(MAX_NOOP_WORKOUTS as i64 + 1)
    .fetch_all(pool)
    .await
    .map_err(anyhow::Error::from)?;
    if records.len() > MAX_NOOP_WORKOUTS {
        return Err(AppError::Unprocessable(format!(
            "More than {MAX_NOOP_WORKOUTS} NOOP workouts start on {day}; refusing to return a \
             truncated day"
        )));
    }
    records
        .into_iter()
        .map(|record| {
            // Starts are bounded by CalendarDay. Validate ends before merge arithmetic so a
            // malformed pushed timestamp cannot overflow or disappear during deduplication.
            calendar::from_unix(record.end_ts).ok_or_else(|| {
                anyhow::anyhow!("NOOP timestamp {} is out of range", record.end_ts)
            })?;
            Ok(WorkoutRow {
                device_id: record.device_id,
                start_ts: record.start_ts,
                end_ts: record.end_ts,
                origin: WorkoutOrigin::classify(&record.source),
                sport: record.sport,
                duration_s: record.duration_s,
                energy_kcal: record.energy_kcal,
                avg_hr: record.avg_hr,
                max_hr: record.max_hr,
                strain: record.strain,
                distance_m: record.distance_m,
                steps: record.steps,
                notes: record.notes,
                has_zones: record.has_zones,
            })
        })
        .collect()
}

fn noop_workout(row: &WorkoutRow) -> Result<NoopWorkout> {
    Ok(NoopWorkout {
        start: calendar::iso_from_unix(row.start_ts)?,
        end: calendar::iso_from_unix(row.end_ts)?,
        sport: row.sport.clone(),
        origin: row.origin,
        duration_min: row.duration_s.map(|seconds| seconds / 60.0),
        avg_hr_bpm: row.avg_hr,
        max_hr_bpm: row.max_hr,
        strain_score: row.strain,
        energy_kcal: row.energy_kcal,
        distance_m: row.distance_m,
        steps: row.steps,
        notes: row.notes.clone(),
        source: row.device_id.clone(),
    })
}
