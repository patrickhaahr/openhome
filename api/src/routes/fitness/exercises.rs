use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};

use super::{EXERCISE_REPLAY, accepted_target, double_option};
use crate::error::{AppError, Result};
use crate::services::exercise_progress::{self, CombinedProgress, Period, Selection};
use crate::services::nutrition::meal_logs::parse_request_id;

#[derive(Debug, Deserialize)]
pub struct ListExercises {
    category: Option<String>,
    muscle_group: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateExercise {
    request_id: Option<String>,
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
            "Invalid category '{category}': must be 'calisthenics' or 'gym'"
        )));
    }
    Ok(())
}

fn map_db_error(e: sqlx::Error, name: &str) -> AppError {
    match e {
        sqlx::Error::Database(db_err) if db_err.is_unique_violation() => {
            AppError::Conflict(format!("Exercise '{name}' already exists"))
        }
        other => AppError::Internal(anyhow::anyhow!("Database error: {other}")),
    }
}

pub(super) async fn list_exercises(
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
    .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to list exercises: {e}")))?;

    Ok(Json(exercises))
}

pub(super) async fn get_exercise(
    State(state): State<crate::AppState>,
    Path(id): Path<i64>,
) -> Result<Json<Exercise>> {
    fetch_exercise(&state, id).await.map(Json)
}

async fn fetch_exercise(state: &crate::AppState, id: i64) -> Result<Exercise> {
    sqlx::query_as!(
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
    .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to fetch exercise: {e}")))?
    .ok_or_else(|| AppError::NotFound(format!("Exercise with id {id} not found")))
}

struct ExerciseReceipt {
    request_id: String,
    request: String,
}

impl ExerciseReceipt {
    fn of(request_id: &str, payload: &CreateExercise) -> Result<Self> {
        #[derive(Serialize)]
        struct Normalized<'a> {
            name: &'a str,
            category: &'a str,
            #[serde(skip_serializing_if = "Option::is_none")]
            muscle_group: Option<&'a str>,
            #[serde(skip_serializing_if = "Option::is_none")]
            equipment: Option<&'a str>,
        }
        let request = serde_json::to_string(&Normalized {
            name: &payload.name,
            category: &payload.category,
            muscle_group: payload.muscle_group.as_deref(),
            equipment: payload.equipment.as_deref(),
        })
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to encode receipt: {e}")))?;
        Ok(Self {
            request_id: parse_request_id(request_id).map_err(AppError::Validation)?,
            request,
        })
    }
}

async fn replay_exercise(
    state: &crate::AppState,
    receipt: &ExerciseReceipt,
) -> Result<Option<Exercise>> {
    let Some(stored) = sqlx::query!(
        "SELECT exercise_id, request FROM exercise_receipts WHERE request_id = $1",
        receipt.request_id
    )
    .fetch_optional(&state.db)
    .await
    .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to read exercise receipt: {e}")))?
    else {
        return Ok(None);
    };
    let exercise_id = accepted_target(
        &stored.request,
        stored.exercise_id,
        &receipt.request,
        &EXERCISE_REPLAY,
    )?;
    fetch_exercise(state, exercise_id).await.map(Some)
}

pub(super) async fn create_exercise(
    State(state): State<crate::AppState>,
    Json(payload): Json<CreateExercise>,
) -> Result<(StatusCode, Json<Exercise>)> {
    validate_category(&payload.category)?;
    let receipt = payload
        .request_id
        .as_deref()
        .map(|request_id| ExerciseReceipt::of(request_id, &payload))
        .transpose()?;
    if let Some(receipt) = &receipt
        && let Some(exercise) = replay_exercise(&state, receipt).await?
    {
        return Ok((StatusCode::OK, Json(exercise)));
    }

    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to start transaction: {e}")))?;

    let inserted = sqlx::query_as!(
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
    .fetch_one(&mut *tx)
    .await;
    let exercise = match inserted {
        Ok(exercise) => exercise,
        Err(error) => {
            tx.rollback().await.map_err(|e| {
                AppError::Internal(anyhow::anyhow!("Failed to roll back transaction: {e}"))
            })?;
            if let Some(receipt) = &receipt
                && let Some(exercise) = replay_exercise(&state, receipt).await?
            {
                return Ok((StatusCode::OK, Json(exercise)));
            }
            return Err(map_db_error(error, &payload.name));
        }
    };

    if let Some(receipt) = &receipt {
        let claimed = sqlx::query_scalar!(
            r#"
            INSERT INTO exercise_receipts (request_id, exercise_id, request)
            VALUES ($1, $2, $3)
            ON CONFLICT (request_id) DO NOTHING
            RETURNING request_id
            "#,
            receipt.request_id,
            exercise.id,
            receipt.request
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to store receipt: {e}")))?;
        if claimed.is_none() {
            tx.rollback().await.map_err(|e| {
                AppError::Internal(anyhow::anyhow!("Failed to roll back transaction: {e}"))
            })?;
            let exercise = replay_exercise(&state, receipt).await?.ok_or_else(|| {
                AppError::Conflict("Retry receipt unavailable; list exercises first".into())
            })?;
            return Ok((StatusCode::OK, Json(exercise)));
        }
    }
    tx.commit()
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to commit transaction: {e}")))?;

    Ok((StatusCode::CREATED, Json(exercise)))
}

pub(super) async fn update_exercise(
    State(state): State<crate::AppState>,
    Path(id): Path<i64>,
    Json(payload): Json<UpdateExercise>,
) -> Result<Json<Exercise>> {
    if let Some(category) = &payload.category {
        validate_category(category)?;
    }

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
    .ok_or_else(|| AppError::NotFound(format!("Exercise with id {id} not found")))?;

    Ok(Json(exercise))
}

pub(super) async fn delete_exercise(
    State(state): State<crate::AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode> {
    let result = sqlx::query!("DELETE FROM exercises WHERE id = $1", id)
        .execute(&state.db)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(db_err) if db_err.is_foreign_key_violation() => {
                AppError::Conflict(format!(
                    "Exercise with id {id} is used by logged workouts and cannot be deleted"
                ))
            }
            other => AppError::Internal(anyhow::anyhow!("Failed to delete exercise: {other}")),
        })?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!(
            "Exercise with id {id} not found"
        )));
    }

    Ok(StatusCode::NO_CONTENT)
}

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

pub(super) async fn exercise_progress(
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
    .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to fetch exercise: {e}")))?
    .ok_or_else(|| AppError::NotFound(format!("Exercise with id {id} not found")))?;

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
    .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to compute progress: {e}")))?;

    Ok(Json(ExerciseProgress { exercise, data }))
}

#[derive(Debug, Deserialize)]
pub struct CombinedProgressQuery {
    from: Option<String>,
    to: Option<String>,
    exercise_ids: Option<String>,
}

pub(super) async fn combined_progress(
    State(state): State<crate::AppState>,
    Query(params): Query<CombinedProgressQuery>,
) -> Result<Json<CombinedProgress>> {
    let (Some(from), Some(to)) = (params.from.as_deref(), params.to.as_deref()) else {
        return Err(AppError::Validation(
            "Progress needs 'from' and 'to' days written YYYY-MM-DD".to_string(),
        ));
    };
    let period = Period::parse(from, to)?;
    let selection = Selection::parse(params.exercise_ids.as_deref())?;
    Ok(Json(
        exercise_progress::combined_progress(&state.db, period, selection).await?,
    ))
}
