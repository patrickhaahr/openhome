use crate::error::{AppError, Result};

#[derive(Debug, serde::Serialize)]
struct SetRow {
    id: i64,
    set_number: i64,
    reps: Option<i64>,
    weight_kg: Option<f64>,
    duration_seconds: Option<i64>,
    rpe: Option<i64>,
    notes: Option<String>,
}

#[derive(Debug, serde::Serialize)]
struct WorkoutExercise {
    id: i64,
    order_index: i64,
    notes: Option<String>,
    exercise: ExerciseSummary,
    sets: Vec<SetRow>,
}

#[derive(Debug, sqlx::FromRow, serde::Serialize)]
struct ExerciseSummary {
    id: i64,
    name: String,
    category: String,
}

#[derive(Debug, serde::Serialize)]
pub struct WorkoutDetail {
    id: i64,
    date: String,
    name: Option<String>,
    notes: Option<String>,
    /// Where a client that downloads this Workout continues its edit revisions from.
    #[serde(skip_serializing_if = "Option::is_none")]
    client_revision: Option<i64>,
    exercises: Vec<WorkoutExercise>,
}

async fn fetch_exercise_summaries(
    db: &mut sqlx::SqliteConnection,
    workout_id: i64,
) -> Result<Vec<WorkoutExercise>> {
    let entries = sqlx::query_as!(
        EntryRow,
        r#"
        SELECT we.id                        AS "entry_id!",
               we.order_index,
               we.notes                     AS entry_notes,
               e.id                         AS "exercise_id_num!",
               e.name                       AS "exercise_name!",
               e.category                   AS "exercise_category!"
        FROM workout_exercises we
        JOIN exercises e ON e.id = we.exercise_id
        WHERE we.workout_id = $1
        ORDER BY we.order_index, we.id
        "#,
        workout_id
    )
    .fetch_all(&mut *db)
    .await
    .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to fetch workout exercises: {e}")))?;

    let mut result = Vec::with_capacity(entries.len());
    for entry in entries {
        let sets = sqlx::query_as!(
            SetRow,
            r#"
            SELECT id AS "id!", set_number, reps, weight_kg, duration_seconds, rpe, notes
            FROM sets
            WHERE workout_exercise_id = $1
            ORDER BY set_number, id
            "#,
            entry.entry_id
        )
        .fetch_all(&mut *db)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to fetch sets: {e}")))?;

        result.push(WorkoutExercise {
            id: entry.entry_id,
            order_index: entry.order_index,
            notes: entry.entry_notes,
            exercise: ExerciseSummary {
                id: entry.exercise_id_num,
                name: entry.exercise_name,
                category: entry.exercise_category,
            },
            sets,
        });
    }
    Ok(result)
}

#[derive(Debug, sqlx::FromRow)]
struct EntryRow {
    entry_id: i64,
    order_index: i64,
    entry_notes: Option<String>,
    #[allow(dead_code)]
    exercise_id_num: i64,
    exercise_name: String,
    exercise_category: String,
}

pub(super) async fn get_workout_detail(state: &crate::AppState, id: i64) -> Result<WorkoutDetail> {
    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to start transaction: {e}")))?;

    let workout = sqlx::query!(
        r#"
        SELECT CAST(id AS INTEGER)  AS id,
               CAST(date AS TEXT)   AS date,
               name,
               notes,
               client_revision
        FROM workouts
        WHERE id = $1
        "#,
        id
    )
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to fetch workout: {e}")))?
    .ok_or_else(|| AppError::NotFound(format!("Workout with id {id} not found")))?;

    let exercises = fetch_exercise_summaries(&mut tx, id).await?;
    tx.commit()
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to commit transaction: {e}")))?;

    Ok(WorkoutDetail {
        id: workout.id,
        date: workout.date,
        name: workout.name,
        notes: workout.notes,
        client_revision: workout.client_revision,
        exercises,
    })
}
