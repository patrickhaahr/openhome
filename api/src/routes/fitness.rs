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
    #[serde(default, with = "double_option")]
    muscle_group: Option<Option<String>>,
    #[serde(default, with = "double_option")]
    equipment: Option<Option<String>>,
}

/// Deserializes `Option<Option<T>>` so absent and explicit `null` differ:
/// absent -> `None` (keep), `null` -> `Some(None)` (clear), value -> `Some(Some(v))`.
/// serde_json signals a present `null` through `visit_none` (`visit_unit` in
/// other self-describing formats); an absent field never reaches the
/// deserializer (`#[serde(default)]` supplies `None`).
mod double_option {
    use serde::{Deserialize, Deserializer, de};
    use std::marker::PhantomData;

    pub fn deserialize<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
    where
        D: Deserializer<'de>,
        Option<T>: Deserialize<'de>,
    {
        struct Visitor<T>(PhantomData<T>);

        impl<'de, T> de::Visitor<'de> for Visitor<T>
        where
            Option<T>: Deserialize<'de>,
        {
            type Value = Option<Option<T>>;

            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("an optional value")
            }

            fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
                Ok(Some(None))
            }

            fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
                Ok(Some(None))
            }

            fn visit_some<D>(self, d: D) -> Result<Self::Value, D::Error>
            where
                D: Deserializer<'de>,
            {
                Option::<T>::deserialize(d).map(Some)
            }
        }

        deserializer.deserialize_option(Visitor(PhantomData))
    }
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
        .route("/api/exercises/{id}/progress", get(exercise_progress))
        .merge(workout_router())
        .merge(metrics_router())
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

    // Clearable partial update: absent field keeps the current value, explicit
    // null sets NULL (documented in api/AGENTS.md); rename conflicts surface
    // as unique violations. Single statement keeps concurrent PATCHes atomic.
    let muscle_group_set = payload.muscle_group.is_some();
    let muscle_group = payload.muscle_group.flatten();
    let equipment_set = payload.equipment.is_some();
    let equipment = payload.equipment.flatten();
    let exercise = sqlx::query_as!(
        Exercise,
        r#"
        UPDATE exercises
        SET name = COALESCE($1, name),
            category = COALESCE($2, category),
            muscle_group = CASE WHEN $3 THEN $4 ELSE muscle_group END,
            equipment = CASE WHEN $5 THEN $6 ELSE equipment END
        WHERE id = $7
        RETURNING id, name, category, muscle_group, equipment
        "#,
        payload.name,
        payload.category,
        muscle_group_set,
        muscle_group,
        equipment_set,
        equipment,
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
    #[serde(default, with = "double_option")]
    name: Option<Option<String>>,
    #[serde(default, with = "double_option")]
    notes: Option<Option<String>>,
    #[serde(default, with = "double_option")]
    body_weight_kg: Option<Option<f64>>,
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

    // Clearable partial update: absent field keeps the current value, explicit
    // null sets NULL (nullable fields only; date is NOT NULL so null = keep).
    // Exercises/sets: when 'exercises' is present, replace the whole nested
    // list (delete children, re-insert). When absent, keep the current entries.
    let name_set = payload.name.is_some();
    let name = payload.name.flatten();
    let notes_set = payload.notes.is_some();
    let notes = payload.notes.flatten();
    let body_weight_set = payload.body_weight_kg.is_some();
    let body_weight_kg = payload.body_weight_kg.flatten();
    let result = sqlx::query!(
        r#"
        UPDATE workouts
        SET date = COALESCE($1, date),
            name = CASE WHEN $2 THEN $3 ELSE name END,
            notes = CASE WHEN $4 THEN $5 ELSE notes END,
            body_weight_kg = CASE WHEN $6 THEN $7 ELSE body_weight_kg END,
            updated_at = CURRENT_TIMESTAMP
        WHERE id = $8
        "#,
        date,
        name_set,
        name,
        notes_set,
        notes,
        body_weight_set,
        body_weight_kg,
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

// ---------------------------------------------------------------------------
// Progress (#8)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct ProgressQuery {
    from: Option<String>,
    to: Option<String>,
}

#[derive(Debug, serde::Serialize)]
struct ProgressPoint {
    date: String,
    best_reps: Option<i64>,
    best_weight_kg: Option<f64>,
    total_volume_kg: Option<f64>,
    best_rpe: Option<i64>,
    estimated_1rm_kg: Option<f64>,
}

#[derive(Debug, serde::Serialize)]
pub struct ExerciseProgress {
    exercise: Exercise,
    data: Vec<ProgressPoint>,
}

async fn exercise_progress(
    State(state): State<crate::AppState>,
    Path(id): Path<i64>,
    Query(params): Query<ProgressQuery>,
) -> Result<Json<ExerciseProgress>> {
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

    // One row per workout date. Volume = reps * weight_kg (added weight; null
    // weight contributes zero), duration_seconds * weight_kg for holds.
    // Epley 1RM comes from the best set of the day among sets with both
    // reps and weight; observed data only, no interpolation.
    let data = sqlx::query_as!(
        ProgressPoint,
        r#"
        SELECT CAST(w.date AS TEXT) AS "date!: String",
               CAST(MAX(s.reps) AS INTEGER) AS "best_reps?: i64",
               CAST(MAX(s.weight_kg) AS REAL) AS "best_weight_kg?: f64",
               CAST(SUM(COALESCE(s.reps, s.duration_seconds) * COALESCE(s.weight_kg, 0.0)) AS REAL) AS "total_volume_kg?: f64",
               CAST(MAX(s.rpe) AS INTEGER) AS "best_rpe?: i64",
               CAST(MAX(CASE WHEN s.reps IS NOT NULL AND s.weight_kg IS NOT NULL
                             THEN s.weight_kg * (1.0 + s.reps / 30.0) END) AS REAL) AS "estimated_1rm_kg?: f64"
        FROM sets s
        JOIN workout_exercises we ON we.id = s.workout_exercise_id
        JOIN workouts w ON w.id = we.workout_id
        WHERE we.exercise_id = $1
          AND ($2 IS NULL OR w.date >= $2)
          AND ($3 IS NULL OR w.date <= $3)
        GROUP BY w.date
        ORDER BY w.date
        "#,
        id,
        params.from,
        params.to
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to compute progress: {}", e)))?;

    Ok(Json(ExerciseProgress { exercise, data }))
}

// ---------------------------------------------------------------------------
// Body weight (#8)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct ListBodyWeight {
    from: Option<String>,
    to: Option<String>,
    limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct CreateBodyWeight {
    date: String,
    weight_kg: f64,
}

#[derive(Debug, serde::Serialize)]
pub struct BodyWeightEntry {
    id: i64,
    date: String,
    weight_kg: f64,
}

async fn list_body_weight(
    State(state): State<crate::AppState>,
    Query(params): Query<ListBodyWeight>,
) -> Result<Json<Vec<BodyWeightEntry>>> {
    let limit = params.limit.unwrap_or(100).clamp(1, 500);
    let entries = sqlx::query_as!(
        BodyWeightEntry,
        r#"
        SELECT CAST(id AS INTEGER) AS "id!: i64",
               CAST(date AS TEXT)  AS "date!: String",
               weight_kg
        FROM body_weight
        WHERE ($1 IS NULL OR date >= $1)
          AND ($2 IS NULL OR date <= $2)
        ORDER BY date, id
        LIMIT $3
        "#,
        params.from,
        params.to,
        limit
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to list body weight: {}", e)))?;

    Ok(Json(entries))
}

async fn create_body_weight(
    State(state): State<crate::AppState>,
    Json(payload): Json<CreateBodyWeight>,
) -> Result<(StatusCode, Json<BodyWeightEntry>)> {
    let date = validate_date(&payload.date)?;
    if payload.weight_kg <= 0.0 {
        return Err(AppError::Validation(
            "weight_kg must be greater than zero".to_string(),
        ));
    }

    let entry = sqlx::query_as!(
        BodyWeightEntry,
        r#"
        INSERT INTO body_weight (date, weight_kg)
        VALUES ($1, $2)
        RETURNING CAST(id AS INTEGER) AS "id!: i64",
                  CAST(date AS TEXT)  AS "date!: String",
                  weight_kg
        "#,
        date,
        payload.weight_kg
    )
    .fetch_one(&state.db)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(db_err) if db_err.is_unique_violation() => {
            AppError::Conflict(format!("Body weight for date '{}' already recorded", date))
        }
        other => AppError::Internal(anyhow::anyhow!("Failed to record body weight: {}", other)),
    })?;

    Ok((StatusCode::CREATED, Json(entry)))
}

// ---------------------------------------------------------------------------
// Profile (#8)
// ---------------------------------------------------------------------------

#[derive(Debug, serde::Serialize)]
pub struct Profile {
    height_cm: Option<f64>,
    sex: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProfile {
    #[serde(default, with = "double_option")]
    height_cm: Option<Option<f64>>,
    #[serde(default, with = "double_option")]
    sex: Option<Option<String>>,
}

async fn get_profile(State(state): State<crate::AppState>) -> Result<Json<Profile>> {
    let row = sqlx::query_as!(
        Profile,
        r#"
        SELECT height_cm, sex
        FROM profile
        WHERE id = 1
        "#
    )
    .fetch_optional(&state.db)
    .await
    .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to fetch profile: {}", e)))?;

    // Not-configured response: the single row simply carries null fields.
    Ok(Json(row.unwrap_or(Profile {
        height_cm: None,
        sex: None,
    })))
}

async fn update_profile(
    State(state): State<crate::AppState>,
    Json(payload): Json<UpdateProfile>,
) -> Result<Json<Profile>> {
    if let Some(Some(height)) = payload.height_cm
        && height <= 0.0
    {
        return Err(AppError::Validation(
            "height_cm must be greater than zero".to_string(),
        ));
    }

    // Clearable upsert: absent field keeps the current value (stays NULL on
    // first create), explicit null sets NULL (documented in api/AGENTS.md).
    // Single statement keeps concurrent first PATCHes atomic.
    let height_set = payload.height_cm.is_some();
    let height_cm = payload.height_cm.flatten();
    let sex_set = payload.sex.is_some();
    let sex = payload.sex.flatten();
    let profile = sqlx::query_as!(
        Profile,
        r#"
        INSERT INTO profile (id, height_cm, sex)
        VALUES (1, $1, $2)
        ON CONFLICT(id) DO UPDATE SET
            height_cm = CASE WHEN $3 THEN $1 ELSE height_cm END,
            sex = CASE WHEN $4 THEN $2 ELSE sex END,
            updated_at = CURRENT_TIMESTAMP
        RETURNING height_cm, sex
        "#,
        height_cm,
        sex,
        height_set,
        sex_set
    )
    .fetch_one(&state.db)
    .await
    .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to update profile: {}", e)))?;

    Ok(Json(profile))
}

pub fn metrics_router() -> Router<crate::AppState> {
    Router::new()
        .route(
            "/api/body_weight",
            get(list_body_weight).post(create_body_weight),
        )
        .route("/api/profile", get(get_profile).patch(update_profile))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Deserialize)]
    struct PatchSubject {
        name: Option<String>,
        #[serde(default, with = "double_option")]
        clearable: Option<Option<String>>,
    }

    #[test]
    fn double_option_distinguishes_absent_and_null() {
        // absent field: keep
        let s: PatchSubject = serde_json::from_str(r#"{"name": "x"}"#).unwrap();
        assert_eq!(s.clearable, None);
        assert_eq!(s.name, Some("x".to_string()));

        // explicit null: clear
        let s: PatchSubject = serde_json::from_str(r#"{"name": "x", "clearable": null}"#).unwrap();
        assert_eq!(s.clearable, Some(None));

        // present value: set
        let s: PatchSubject = serde_json::from_str(r#"{"name": "x", "clearable": "y"}"#).unwrap();
        assert_eq!(s.clearable, Some(Some("y".to_string())));
    }
}
