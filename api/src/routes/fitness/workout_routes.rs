use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::get,
};
use serde::{Deserialize, Serialize};

use super::workout_detail::{WorkoutDetail, get_workout_detail};
use super::{WORKOUT_REPLAY, accepted_target, double_option, parse_client_revision, validate_date};
use crate::error::{AppError, Result};
use crate::services::nutrition::meal_logs::parse_request_id;

// ---------------------------------------------------------------------------
// Workouts (#7)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct ListWorkouts {
    from: Option<String>,
    to: Option<String>,
    limit: Option<i64>,
}

// Absent fields stay out of the Serialize form, which a Workout receipt compares on retry, so a
// field added later cannot make an earlier receipt mismatch its own retry.
#[derive(Debug, Deserialize, Serialize)]
struct SetInput {
    set_number: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    reps: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    weight_kg: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    duration_seconds: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rpe: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    notes: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct WorkoutExerciseInput {
    exercise_id: i64,
    order_index: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    notes: Option<String>,
    sets: Vec<SetInput>,
}

#[derive(Debug, Deserialize)]
pub struct CreateWorkout {
    /// Makes this create safe to retry: see [`replay_workout`]. Without it every call creates.
    request_id: Option<String>,
    date: String,
    name: Option<String>,
    notes: Option<String>,
    exercises: Vec<WorkoutExerciseInput>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateWorkout {
    /// Makes this edit safe to replay: see [`update_workout`]. Without it every call applies.
    client_revision: Option<i64>,
    date: Option<String>,
    #[serde(default, with = "double_option")]
    name: Option<Option<String>>,
    #[serde(default, with = "double_option")]
    notes: Option<Option<String>>,
    exercises: Option<Vec<WorkoutExerciseInput>>,
}

#[derive(Debug, serde::Serialize)]
pub struct WorkoutSummary {
    id: i64,
    date: String,
    name: Option<String>,
}

/// A set must have reps or duration (timed hold), never both null. RPE is 1-10.
/// `weight_kg` is *added* weight: null for bodyweight, not zero.
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
            other => {
                AppError::Internal(anyhow::anyhow!("Failed to insert exercise entry: {other}"))
            }
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
            .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to insert set: {e}")))?;
        }
    }
    Ok(())
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

pub(super) async fn list_workouts(
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
    .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to list workouts: {e}")))?;

    Ok(Json(workouts))
}

pub(super) async fn get_workout(
    State(state): State<crate::AppState>,
    Path(id): Path<i64>,
) -> Result<Json<WorkoutDetail>> {
    let workout = get_workout_detail(&state, id).await?;
    Ok(Json(workout))
}

/// A Workout create's retry identity: its `request_id` and the arguments a retry must repeat.
struct WorkoutReceipt {
    request_id: String,
    request: String,
}

impl WorkoutReceipt {
    fn of(request_id: &str, date: &str, payload: &CreateWorkout) -> Result<Self> {
        #[derive(Serialize)]
        struct Normalized<'a> {
            date: &'a str,
            #[serde(skip_serializing_if = "Option::is_none")]
            name: Option<&'a str>,
            #[serde(skip_serializing_if = "Option::is_none")]
            notes: Option<&'a str>,
            exercises: &'a [WorkoutExerciseInput],
        }
        let request = serde_json::to_string(&Normalized {
            date,
            name: payload.name.as_deref(),
            notes: payload.notes.as_deref(),
            exercises: &payload.exercises,
        })
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to encode receipt: {e}")))?;
        Ok(Self {
            request_id: parse_request_id(request_id).map_err(AppError::Validation)?,
            request,
        })
    }
}

/// Follows the `request_id` receipt pattern: a retry answers with the Workout it already created,
/// the same `request_id` with changed arguments is a conflict, and a deleted Workout stays deleted.
/// `None` means this `request_id` has not been accepted.
async fn replay_workout(
    state: &crate::AppState,
    receipt: &WorkoutReceipt,
) -> Result<Option<WorkoutDetail>> {
    let Some(stored) = sqlx::query!(
        "SELECT workout_id, request FROM workout_receipts WHERE request_id = $1",
        receipt.request_id
    )
    .fetch_optional(&state.db)
    .await
    .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to read workout receipt: {e}")))?
    else {
        return Ok(None);
    };
    let workout_id = accepted_target(
        &stored.request,
        stored.workout_id,
        &receipt.request,
        &WORKOUT_REPLAY,
    )?;
    get_workout_detail(state, workout_id).await.map(Some)
}

pub(super) async fn create_workout(
    State(state): State<crate::AppState>,
    Json(payload): Json<CreateWorkout>,
) -> Result<(StatusCode, Json<WorkoutDetail>)> {
    let date = validate_date(&payload.date)?;
    validate_workout_exercises(&payload.exercises)?;
    let receipt = payload
        .request_id
        .as_deref()
        .map(|request_id| WorkoutReceipt::of(request_id, &date, &payload))
        .transpose()?;
    if let Some(receipt) = &receipt
        && let Some(workout) = replay_workout(&state, receipt).await?
    {
        return Ok((StatusCode::OK, Json(workout)));
    }

    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to start transaction: {e}")))?;

    let workout_id = sqlx::query!(
        r#"
        INSERT INTO workouts (date, name, notes)
        VALUES ($1, $2, $3)
        "#,
        date,
        payload.name,
        payload.notes
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to create workout: {e}")))?
    .last_insert_rowid();

    if let Some(receipt) = &receipt {
        // The insert above holds the write lock, so simultaneous retries serialize here.
        let claimed = sqlx::query_scalar!(
            r#"
            INSERT INTO workout_receipts (request_id, workout_id, request)
            VALUES ($1, $2, $3)
            ON CONFLICT (request_id) DO NOTHING
            RETURNING request_id
            "#,
            receipt.request_id,
            workout_id,
            receipt.request
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to store receipt: {e}")))?;
        if claimed.is_none() {
            tx.rollback().await.map_err(|e| {
                AppError::Internal(anyhow::anyhow!("Failed to roll back transaction: {e}"))
            })?;
            let workout = replay_workout(&state, receipt).await?.ok_or_else(|| {
                AppError::Conflict("Retry receipt unavailable; list workouts first".into())
            })?;
            return Ok((StatusCode::OK, Json(workout)));
        }
    }

    insert_exercises(&mut tx, workout_id, payload.exercises).await?;
    tx.commit()
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to commit transaction: {e}")))?;

    let workout = get_workout_detail(&state, workout_id).await?;
    Ok((StatusCode::CREATED, Json(workout)))
}

/// With a `client_revision`, an edit applies only if it is newer than the last one applied. An older
/// or repeated revision is an edit this client has already superseded or delivered, so it changes
/// nothing and answers with the current Workout: a retry after a lost response and a late delivery
/// both end as acknowledged, without restoring an older revision.
pub(super) async fn update_workout(
    State(state): State<crate::AppState>,
    Path(id): Path<i64>,
    Json(payload): Json<UpdateWorkout>,
) -> Result<Json<WorkoutDetail>> {
    let date = payload.date.as_deref().map(validate_date).transpose()?;
    if let Some(exercises) = &payload.exercises {
        validate_workout_exercises(exercises)?;
    }
    let client_revision = parse_client_revision(payload.client_revision)?;

    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to start transaction: {e}")))?;

    // Clearable partial update: absent field keeps the current value, explicit
    // null sets NULL (nullable fields only; date is NOT NULL so null = keep).
    // Exercises/sets: when 'exercises' is present, replace the whole nested
    // list (delete children, re-insert). When absent, keep the current entries.
    let name_set = payload.name.is_some();
    let name = payload.name.flatten();
    let notes_set = payload.notes.is_some();
    let notes = payload.notes.flatten();
    let result = sqlx::query!(
        r#"
        UPDATE workouts
        SET date = COALESCE($1, date),
            name = CASE WHEN $2 THEN $3 ELSE name END,
            notes = CASE WHEN $4 THEN $5 ELSE notes END,
            client_revision = COALESCE($7, client_revision),
            updated_at = CURRENT_TIMESTAMP
        WHERE id = $6
          AND ($7 IS NULL OR client_revision IS NULL OR client_revision < $7)
        "#,
        date,
        name_set,
        name,
        notes_set,
        notes,
        id,
        client_revision
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to update workout: {e}")))?;

    if result.rows_affected() == 0 {
        tx.rollback().await.map_err(|e| {
            AppError::Internal(anyhow::anyhow!("Failed to roll back transaction: {e}"))
        })?;
        // No such Workout, which this read reports, or a revision it has already moved past.
        return get_workout_detail(&state, id).await.map(Json);
    }

    if let Some(exercises) = payload.exercises {
        sqlx::query!("DELETE FROM workout_exercises WHERE workout_id = $1", id)
            .execute(&mut *tx)
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to replace exercises: {e}")))?;
        insert_exercises(&mut tx, id, exercises).await?;
    }

    tx.commit()
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to commit transaction: {e}")))?;

    let workout = get_workout_detail(&state, id).await?;
    Ok(Json(workout))
}

pub(super) async fn delete_workout(
    State(state): State<crate::AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode> {
    let result = sqlx::query!("DELETE FROM workouts WHERE id = $1", id)
        .execute(&state.db)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to delete workout: {e}")))?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!(
            "Workout with id {id} not found"
        )));
    }

    Ok(StatusCode::NO_CONTENT)
}
