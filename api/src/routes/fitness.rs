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
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to delete exercise: {}", e)))?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!(
            "Exercise with id {} not found",
            id
        )));
    }

    Ok(StatusCode::NO_CONTENT)
}
