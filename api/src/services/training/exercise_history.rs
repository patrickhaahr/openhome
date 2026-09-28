//! Exact logged Set history for one resolved Exercise over up to 90 Copenhagen calendar days.

use serde::Serialize;
use sqlx::SqlitePool;

use crate::error::{AppError, Result};

use super::calendar::{CalendarDay, TIME_ZONE_NAME};
use super::workouts::LoggedSet;

/// Joined Set rows, counting an Exercise entry without Sets as one row.
const MAX_HISTORY_ROWS: usize = 500;

#[derive(Debug, Serialize)]
pub struct ExerciseHistory {
    pub exercise: Exercise,
    pub from_day: String,
    pub to_day: String,
    pub time_zone: &'static str,
    pub workouts: Vec<HistoryWorkout>,
}

#[derive(Debug, Serialize)]
pub struct Exercise {
    pub id: i64,
    pub name: String,
    pub category: String,
}

#[derive(Debug, Serialize)]
pub struct HistoryWorkout {
    pub id: i64,
    pub date: String,
    pub name: Option<String>,
    pub notes: Option<String>,
    /// Repeated entries of the same Exercise stay distinct and in logged order.
    pub entries: Vec<HistoryEntry>,
}

#[derive(Debug, Serialize)]
pub struct HistoryEntry {
    pub notes: Option<String>,
    pub sets: Vec<LoggedSet>,
}

pub async fn exercise_history(
    db: &SqlitePool,
    name: &str,
    from_day: CalendarDay,
    to_day: CalendarDay,
) -> Result<ExerciseHistory> {
    let matches = sqlx::query!(
        "SELECT id, name, category FROM exercises WHERE name = $1 COLLATE NOCASE ORDER BY id LIMIT 2",
        name
    )
    .fetch_all(db)
    .await
    .map_err(anyhow::Error::from)?;
    let exercise = match matches.as_slice() {
        [] => {
            return Err(AppError::NotFound(format!(
                "Exercise '{name}' was not found"
            )));
        }
        [match_] => Exercise {
            id: match_.id,
            name: match_.name.clone(),
            category: match_.category.clone(),
        },
        _ => {
            return Err(AppError::Conflict(format!(
                "Exercise name '{name}' is ambiguous; use a unique Exercise name"
            )));
        }
    };

    let from = from_day.to_string();
    let to = to_day.to_string();
    let limit = MAX_HISTORY_ROWS as i64 + 1;
    let rows = sqlx::query!(
        r#"
        SELECT w.id                 AS "workout_id!: i64",
               w.date               AS "workout_date!: String",
               w.name               AS workout_name,
               w.notes              AS workout_notes,
               we.id                AS "entry_id!: i64",
               we.notes             AS entry_notes,
               s.id                 AS "set_id?: i64",
               s.set_number         AS "set_number?: i64",
               s.reps,
               s.weight_kg,
               s.duration_seconds,
               s.rpe,
               s.notes              AS set_notes
        FROM workout_exercises we
        JOIN workouts w ON w.id = we.workout_id
        LEFT JOIN sets s ON s.workout_exercise_id = we.id
        WHERE we.exercise_id = $1 AND w.date BETWEEN $2 AND $3
        ORDER BY w.date, w.id, we.order_index, we.id, s.set_number, s.id
        LIMIT $4
        "#,
        exercise.id,
        from,
        to,
        limit
    )
    .fetch_all(db)
    .await
    .map_err(anyhow::Error::from)?;
    if rows.len() > MAX_HISTORY_ROWS {
        return Err(AppError::Unprocessable(format!(
            "Exercise history has more than {MAX_HISTORY_ROWS} Set rows; refusing to return a truncated result"
        )));
    }

    let mut workouts: Vec<HistoryWorkout> = Vec::new();
    let mut last_entry_id = None;
    for row in rows {
        if workouts
            .last()
            .is_none_or(|workout| workout.id != row.workout_id)
        {
            workouts.push(HistoryWorkout {
                id: row.workout_id,
                date: row.workout_date,
                name: row.workout_name,
                notes: row.workout_notes,
                entries: Vec::new(),
            });
        }
        let workout = workouts.last_mut().expect("pushed above");
        if last_entry_id != Some(row.entry_id) {
            last_entry_id = Some(row.entry_id);
            workout.entries.push(HistoryEntry {
                notes: row.entry_notes,
                sets: Vec::new(),
            });
        }
        if let (Some(_), Some(set_number)) = (row.set_id, row.set_number) {
            workout
                .entries
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
    }

    Ok(ExerciseHistory {
        exercise,
        from_day: from,
        to_day: to,
        time_zone: TIME_ZONE_NAME,
        workouts,
    })
}
