use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::get,
};
use serde::Deserialize;

use crate::error::{AppError, Result};

#[derive(Debug, Deserialize)]
pub struct ListExercises {
    category: Option<String>,
    muscle_group: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateExercise {
    name: String,
    category: String,
    muscle_group: Option<String>,
    equipment: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateExercise {
    name: Option<String>,
    category: Option<String>,
    muscle_group: Option<String>,
    equipment: Option<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct Exercise {
    id: i64,
    name: String,
    category: String,
    muscle_group: Option<String>,
    equipment: Option<String>,
}

fn validate_category(category: &str) -> Result<()> {
    if category != "calisthenics" && category != "gym" {
        return Err(AppError::Validation(format!(
            "Invalid category '{}': must be 'calisthenics' or 'gym'",
            category
        )));
    }
    Ok(())
}

fn map_db_error(e: sqlx::Error, name: &str) -> AppError {
    match e {
        sqlx::Error::Database(db_err) if db_err.is_unique_violation() => {
            AppError::Conflict(format!("Exercise '{}' already exists", name))
        }
        other => AppError::Internal(anyhow::anyhow!("Database error: {}", other)),
    }
}

pub fn router() -> Router<crate::AppState> {
    Router::new()
        .route("/api/exercises", get(list_exercises).post(create_exercise))
        .route(
            "/api/exercises/{id}",
            get(get_exercise)
                .patch(update_exercise)
                .delete(delete_exercise),
        )
        .merge(workout_router())
}

async fn list_exercises(
    State(state): State<crate::AppState>,
    Query(params): Query<ListExercises>,
) -> Result<Json<Vec<Exercise>>> {
    let exercises = sqlx::query_as!(
        Exercise,
        r#"
        SELECT id, name, category, muscle_group, equipment
        FROM exercises
        WHERE ($1 IS NULL OR category = $1)
          AND ($2 IS NULL OR muscle_group = $2)
        ORDER BY id
        "#,
        params.category,
        params.muscle_group
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to list exercises: {}", e)))?;

    Ok(Json(exercises))
}

async fn get_exercise(
    State(state): State<crate::AppState>,
    Path(id): Path<i64>,
) -> Result<Json<Exercise>> {
    let exercise = sqlx::query_as!(
        Exercise,
        r#"
        SELECT id, name, category, muscle_group, equipment
        FROM exercises
        WHERE id = $1
        "#,
        id
    )
    .fetch_optional(&state.db)
    .await
    .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to fetch exercise: {}", e)))?
    .ok_or_else(|| AppError::NotFound(format!("Exercise with id {} not found", id)))?;

    Ok(Json(exercise))
}

async fn create_exercise(
    State(state): State<crate::AppState>,
    Json(payload): Json<CreateExercise>,
) -> Result<(StatusCode, Json<Exercise>)> {
    validate_category(&payload.category)?;

    let exercise = sqlx::query_as!(
        Exercise,
        r#"
        INSERT INTO exercises (name, category, muscle_group, equipment)
        VALUES ($1, $2, $3, $4)
        RETURNING id, name, category, muscle_group, equipment
        "#,
        payload.name,
        payload.category,
        payload.muscle_group,
        payload.equipment
    )
    .fetch_one(&state.db)
    .await
    .map_err(|e| map_db_error(e, &payload.name))?;

    Ok((StatusCode::CREATED, Json(exercise)))
}

async fn update_exercise(
    State(state): State<crate::AppState>,
    Path(id): Path<i64>,
    Json(payload): Json<UpdateExercise>,
) -> Result<Json<Exercise>> {
    if let Some(category) = &payload.category {
        validate_category(category)?;
    }

    // COALESCE partial update: absent and null fields keep the current value
    // (documented in api/AGENTS.md); rename conflicts surface as unique violations.
    let exercise = sqlx::query_as!(
        Exercise,
        r#"
        UPDATE exercises
        SET name = COALESCE($1, name),
            category = COALESCE($2, category),
            muscle_group = COALESCE($3, muscle_group),
            equipment = COALESCE($4, equipment)
        WHERE id = $5
        RETURNING id, name, category, muscle_group, equipment
        "#,
        payload.name,
        payload.category,
        payload.muscle_group,
        payload.equipment,
        id
    )
    .fetch_optional(&state.db)
    .await
    .map_err(|e| map_db_error(e, payload.name.as_deref().unwrap_or("existing exercise")))?
    .ok_or_else(|| AppError::NotFound(format!("Exercise with id {} not found", id)))?;

    Ok(Json(exercise))
}

async fn delete_exercise(
    State(state): State<crate::AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode> {
    let result = sqlx::query!("DELETE FROM exercises WHERE id = $1", id)
        .execute(&state.db)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(db_err) if db_err.is_foreign_key_violation() => {
                AppError::Conflict(format!(
                    "Exercise with id {} is used by logged workouts and cannot be deleted",
                    id
                ))
            }
            other => AppError::Internal(anyhow::anyhow!("Failed to delete exercise: {}", other)),
        })?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!(
            "Exercise with id {} not found",
            id
        )));
    }

    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------------------
// Workouts (#7)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct ListWorkouts {
    from: Option<String>,
    to: Option<String>,
    limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct SetInput {
    set_number: i64,
    reps: Option<i64>,
    weight_kg: Option<f64>,
    duration_seconds: Option<i64>,
    rpe: Option<i64>,
    notes: Option<String>,
}

#[derive(Debug, Deserialize)]
struct WorkoutExerciseInput {
    exercise_id: i64,
    order_index: i64,
    notes: Option<String>,
    sets: Vec<SetInput>,
}

#[derive(Debug, Deserialize)]
pub struct CreateWorkout {
    date: String,
    name: Option<String>,
    notes: Option<String>,
    body_weight_kg: Option<f64>,
    exercises: Vec<WorkoutExerciseInput>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateWorkout {
    date: Option<String>,
    name: Option<String>,
    notes: Option<String>,
    body_weight_kg: Option<f64>,
    exercises: Option<Vec<WorkoutExerciseInput>>,
}

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
pub struct WorkoutSummary {
    id: i64,
    date: String,
    name: Option<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct WorkoutDetail {
    id: i64,
    date: String,
    name: Option<String>,
    notes: Option<String>,
    body_weight_kg: Option<f64>,
    exercises: Vec<WorkoutExercise>,
}

/// Validates and normalizes to a padded `YYYY-MM-DD` string, since list
/// ordering and `?from=`/`?to=` filtering rely on lexicographic comparison.
fn validate_date(date: &str) -> Result<String> {
    let parsed = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").map_err(|_| {
        AppError::Validation(format!("Invalid date '{}': must be YYYY-MM-DD", date))
    })?;
    Ok(parsed.format("%Y-%m-%d").to_string())
}

/// A set must have reps or duration (timed hold), never both null. RPE is 1-10.
/// weight_kg is *added* weight: null for bodyweight, not zero.
fn validate_set(set: &SetInput) -> Result<()> {
    if set.reps.is_none() && set.duration_seconds.is_none() {
        return Err(AppError::Validation(format!(
            "Set {} must have 'reps' or 'duration_seconds'",
            set.set_number
        )));
    }
    if let Some(rpe) = set.rpe
        && !(1..=10).contains(&rpe)
    {
        return Err(AppError::Validation(format!(
            "Set {} RPE {} is out of range (1-10)",
            set.set_number, rpe
        )));
    }
    Ok(())
}

fn validate_workout_exercises(exercises: &[WorkoutExerciseInput]) -> Result<()> {
    for entry in exercises {
        for set in &entry.sets {
            validate_set(set)?;
        }
    }
    Ok(())
}

async fn insert_exercises(
    tx: &mut sqlx::SqliteConnection,
    workout_id: i64,
    exercises: Vec<WorkoutExerciseInput>,
) -> Result<()> {
    for entry in exercises {
        let exercise_row = sqlx::query!(
            r#"
            INSERT INTO workout_exercises (workout_id, exercise_id, order_index, notes)
            VALUES ($1, $2, $3, $4)
            "#,
            workout_id,
            entry.exercise_id,
            entry.order_index,
            entry.notes
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(db_err) if db_err.is_foreign_key_violation() => {
                AppError::Unprocessable(format!("Exercise with id {} not found", entry.exercise_id))
            }
            other => AppError::Internal(anyhow::anyhow!(
                "Failed to insert exercise entry: {}",
                other
            )),
        })?;
        let workout_exercise_id = exercise_row.last_insert_rowid();

        for set in entry.sets {
            sqlx::query!(
                r#"
                INSERT INTO sets (workout_exercise_id, set_number, reps, weight_kg, duration_seconds, rpe, notes)
                VALUES ($1, $2, $3, $4, $5, $6, $7)
                "#,
                workout_exercise_id,
                set.set_number,
                set.reps,
                set.weight_kg,
                set.duration_seconds,
                set.rpe,
                set.notes
            )
            .execute(&mut *tx)
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to insert set: {}", e)))?;
        }
    }
    Ok(())
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
    .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to fetch workout exercises: {}", e)))?;

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
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to fetch sets: {}", e)))?;

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

async fn get_workout_detail(state: &crate::AppState, id: i64) -> Result<WorkoutDetail> {
    let mut tx =
        state.db.begin().await.map_err(|e| {
            AppError::Internal(anyhow::anyhow!("Failed to start transaction: {}", e))
        })?;

    let workout = sqlx::query!(
        r#"
        SELECT CAST(id AS INTEGER)  AS id,
               CAST(date AS TEXT)   AS date,
               name,
               notes,
               body_weight_kg
        FROM workouts
        WHERE id = $1
        "#,
        id
    )
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to fetch workout: {}", e)))?
    .ok_or_else(|| AppError::NotFound(format!("Workout with id {} not found", id)))?;

    let exercises = fetch_exercise_summaries(&mut tx, id).await?;
    tx.commit()
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to commit transaction: {}", e)))?;

    Ok(WorkoutDetail {
        id: workout.id,
        date: workout.date,
        name: workout.name,
        notes: workout.notes,
        body_weight_kg: workout.body_weight_kg,
        exercises,
    })
}

pub fn workout_router() -> Router<crate::AppState> {
    Router::new()
        .route("/api/workouts", get(list_workouts).post(create_workout))
        .route(
            "/api/workouts/{id}",
            get(get_workout)
                .patch(update_workout)
                .delete(delete_workout),
        )
}

async fn list_workouts(
    State(state): State<crate::AppState>,
    Query(params): Query<ListWorkouts>,
) -> Result<Json<Vec<WorkoutSummary>>> {
    let limit = params.limit.unwrap_or(100).clamp(1, 500);
    let workouts = sqlx::query_as!(
        WorkoutSummary,
        r#"
        SELECT CAST(id AS INTEGER)  AS id,
               CAST(date AS TEXT)   AS date,
               name
        FROM workouts
        WHERE ($1 IS NULL OR date >= $1)
          AND ($2 IS NULL OR date <= $2)
        ORDER BY date DESC, id DESC
        LIMIT $3
        "#,
        params.from,
        params.to,
        limit
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to list workouts: {}", e)))?;

    Ok(Json(workouts))
}

async fn get_workout(
    State(state): State<crate::AppState>,
    Path(id): Path<i64>,
) -> Result<Json<WorkoutDetail>> {
    let workout = get_workout_detail(&state, id).await?;
    Ok(Json(workout))
}

async fn create_workout(
    State(state): State<crate::AppState>,
    Json(payload): Json<CreateWorkout>,
) -> Result<(StatusCode, Json<WorkoutDetail>)> {
    let date = validate_date(&payload.date)?;
    validate_workout_exercises(&payload.exercises)?;

    let mut tx =
        state.db.begin().await.map_err(|e| {
            AppError::Internal(anyhow::anyhow!("Failed to start transaction: {}", e))
        })?;

    let workout_id = sqlx::query!(
        r#"
        INSERT INTO workouts (date, name, notes, body_weight_kg)
        VALUES ($1, $2, $3, $4)
        "#,
        date,
        payload.name,
        payload.notes,
        payload.body_weight_kg
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to create workout: {}", e)))?
    .last_insert_rowid();

    insert_exercises(&mut tx, workout_id, payload.exercises).await?;
    tx.commit()
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to commit transaction: {}", e)))?;

    let workout = get_workout_detail(&state, workout_id).await?;
    Ok((StatusCode::CREATED, Json(workout)))
}

async fn update_workout(
    State(state): State<crate::AppState>,
    Path(id): Path<i64>,
    Json(payload): Json<UpdateWorkout>,
) -> Result<Json<WorkoutDetail>> {
    let date = payload.date.as_deref().map(validate_date).transpose()?;
    if let Some(exercises) = &payload.exercises {
        validate_workout_exercises(exercises)?;
    }

    let mut tx =
        state.db.begin().await.map_err(|e| {
            AppError::Internal(anyhow::anyhow!("Failed to start transaction: {}", e))
        })?;

    // COALESCE partial update: absent and null fields keep the current value.
    // Exercises/sets: when 'exercises' is present, replace the whole nested list
    // (delete children, re-insert). When absent, keep the current entries.
    let result = sqlx::query!(
        r#"
        UPDATE workouts
        SET date = COALESCE($1, date),
            name = COALESCE($2, name),
            notes = COALESCE($3, notes),
            body_weight_kg = COALESCE($4, body_weight_kg),
            updated_at = CURRENT_TIMESTAMP
        WHERE id = $5
        "#,
        date,
        payload.name,
        payload.notes,
        payload.body_weight_kg,
        id
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to update workout: {}", e)))?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!(
            "Workout with id {} not found",
            id
        )));
    }

    if let Some(exercises) = payload.exercises {
        sqlx::query!("DELETE FROM workout_exercises WHERE workout_id = $1", id)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                AppError::Internal(anyhow::anyhow!("Failed to replace exercises: {}", e))
            })?;
        insert_exercises(&mut tx, id, exercises).await?;
    }

    tx.commit()
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to commit transaction: {}", e)))?;

    let workout = get_workout_detail(&state, id).await?;
    Ok(Json(workout))
}

async fn delete_workout(
    State(state): State<crate::AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode> {
    let result = sqlx::query!("DELETE FROM workouts WHERE id = $1", id)
        .execute(&state.db)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to delete workout: {}", e)))?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!(
            "Workout with id {} not found",
            id
        )));
    }

    Ok(StatusCode::NO_CONTENT)
}
