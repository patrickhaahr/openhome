//! Read-only training context for API clients such as the Hermes MCP adapter. The rules live in
//! `services::training`; handlers only validate input.

use axum::{
    Json, Router,
    extract::{Path, State},
    routing::get,
};

use crate::error::{AppError, Result};
use crate::services::training::{self, CalendarDay, RecoveryDay, WorkoutsOnDay};

pub fn router() -> Router<crate::AppState> {
    Router::new()
        .route("/api/training/days/{day}/recovery", get(recovery_day))
        .route("/api/training/days/{day}/workouts", get(workouts_on_day))
}

async fn recovery_day(
    State(state): State<crate::AppState>,
    Path(day): Path<String>,
) -> Result<Json<RecoveryDay>> {
    let day = parse_day(&day)?;
    Ok(Json(training::recovery_day(&state.noop_db, day).await?))
}

async fn workouts_on_day(
    State(state): State<crate::AppState>,
    Path(day): Path<String>,
) -> Result<Json<WorkoutsOnDay>> {
    let day = parse_day(&day)?;
    Ok(Json(
        training::workouts_on_day(&state.db, &state.noop_db, day).await?,
    ))
}

fn parse_day(day: &str) -> Result<CalendarDay> {
    CalendarDay::parse(day).ok_or_else(|| {
        AppError::Validation(format!(
            "Invalid day '{day}': must be a Europe/Copenhagen calendar date written YYYY-MM-DD"
        ))
    })
}
