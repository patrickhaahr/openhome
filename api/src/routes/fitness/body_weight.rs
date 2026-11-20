use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};

use super::{BODY_WEIGHT_REPLAY, accepted_target, validate_date};
use crate::error::{AppError, Result};
use crate::services::nutrition::meal_logs::parse_request_id;

#[derive(Debug, Deserialize)]
pub struct ListBodyWeight {
    from: Option<String>,
    to: Option<String>,
    limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct CreateBodyWeight {
    request_id: Option<String>,
    date: String,
    weight_kg: f64,
}

#[derive(Debug, serde::Serialize)]
pub struct BodyWeightEntry {
    id: i64,
    date: String,
    weight_kg: f64,
}

pub(super) async fn list_body_weight(
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
    .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to list body weight: {e}")))?;

    Ok(Json(entries))
}

struct BodyWeightReceipt {
    request_id: String,
    request: String,
}

impl BodyWeightReceipt {
    fn of(request_id: &str, date: &str, weight_kg: f64) -> Result<Self> {
        #[derive(Serialize)]
        struct Normalized<'a> {
            date: &'a str,
            weight_kg: f64,
        }
        let request = serde_json::to_string(&Normalized { date, weight_kg })
            .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to encode receipt: {e}")))?;
        Ok(Self {
            request_id: parse_request_id(request_id).map_err(AppError::Validation)?,
            request,
        })
    }
}

async fn replay_body_weight(
    state: &crate::AppState,
    receipt: &BodyWeightReceipt,
) -> Result<Option<BodyWeightEntry>> {
    let Some(stored) = sqlx::query!(
        "SELECT body_weight_id, request FROM body_weight_receipts WHERE request_id = $1",
        receipt.request_id
    )
    .fetch_optional(&state.db)
    .await
    .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to read body weight receipt: {e}")))?
    else {
        return Ok(None);
    };
    let body_weight_id = accepted_target(
        &stored.request,
        stored.body_weight_id,
        &receipt.request,
        &BODY_WEIGHT_REPLAY,
    )?;
    sqlx::query_as!(
        BodyWeightEntry,
        r#"
        SELECT CAST(id AS INTEGER) AS "id!: i64",
               CAST(date AS TEXT)  AS "date!: String",
               weight_kg
        FROM body_weight
        WHERE id = $1
        "#,
        body_weight_id
    )
    .fetch_one(&state.db)
    .await
    .map(Some)
    .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to read body weight: {e}")))
}

pub(super) async fn create_body_weight(
    State(state): State<crate::AppState>,
    Json(payload): Json<CreateBodyWeight>,
) -> Result<(StatusCode, Json<BodyWeightEntry>)> {
    let date = validate_date(&payload.date)?;
    if payload.weight_kg <= 0.0 {
        return Err(AppError::Validation(
            "weight_kg must be greater than zero".to_string(),
        ));
    }
    let receipt = payload
        .request_id
        .as_deref()
        .map(|request_id| BodyWeightReceipt::of(request_id, &date, payload.weight_kg))
        .transpose()?;
    if let Some(receipt) = &receipt
        && let Some(entry) = replay_body_weight(&state, receipt).await?
    {
        return Ok((StatusCode::OK, Json(entry)));
    }

    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to start transaction: {e}")))?;

    let inserted = sqlx::query_as!(
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
    .fetch_one(&mut *tx)
    .await;
    let entry = match inserted {
        Ok(entry) => entry,
        Err(error) => {
            tx.rollback().await.map_err(|e| {
                AppError::Internal(anyhow::anyhow!("Failed to roll back transaction: {e}"))
            })?;
            if let Some(receipt) = &receipt
                && let Some(entry) = replay_body_weight(&state, receipt).await?
            {
                return Ok((StatusCode::OK, Json(entry)));
            }
            return Err(match error {
                sqlx::Error::Database(db_err) if db_err.is_unique_violation() => {
                    AppError::Conflict(format!("Body weight for date '{date}' already recorded"))
                }
                other => {
                    AppError::Internal(anyhow::anyhow!("Failed to record body weight: {other}"))
                }
            });
        }
    };

    if let Some(receipt) = &receipt {
        let claimed = sqlx::query_scalar!(
            r#"
            INSERT INTO body_weight_receipts (request_id, body_weight_id, request)
            VALUES ($1, $2, $3)
            ON CONFLICT (request_id) DO NOTHING
            RETURNING request_id
            "#,
            receipt.request_id,
            entry.id,
            receipt.request
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to store receipt: {e}")))?;
        if claimed.is_none() {
            tx.rollback().await.map_err(|e| {
                AppError::Internal(anyhow::anyhow!("Failed to roll back transaction: {e}"))
            })?;
            let entry = replay_body_weight(&state, receipt).await?.ok_or_else(|| {
                AppError::Conflict("Retry receipt unavailable; list body weight first".into())
            })?;
            return Ok((StatusCode::OK, Json(entry)));
        }
    }
    tx.commit()
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to commit transaction: {e}")))?;

    Ok((StatusCode::CREATED, Json(entry)))
}
