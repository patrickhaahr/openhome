//! Read-only training context for API clients such as the Hermes MCP adapter. The rules live in
//! `services::training`; handlers only validate input.

use axum::{
    Json, Router,
    extract::{Path, State},
    routing::get,
};

use crate::error::{AppError, Result};
use crate::services::training::{
    self, CalendarDay, ExerciseHistory, RecentSleepNights, RecoveryDay, WorkoutsOnDay,
};

pub fn router() -> Router<crate::AppState> {
    Router::new()
        .route("/api/training/days/{day}/recovery", get(recovery_day))
        .route("/api/training/days/{day}/workouts", get(workouts_on_day))
        .route(
            "/api/training/sleep/recent/{count}",
            get(recent_sleep_nights),
        )
        .route(
            "/api/training/exercises/{exercise_name}/history/{from_day}/{to_day}",
            get(exercise_history),
        )
}

async fn recent_sleep_nights(
    State(state): State<crate::AppState>,
    Path(count): Path<String>,
) -> Result<Json<RecentSleepNights>> {
    let count = count
        .parse::<u8>()
        .ok()
        .filter(|count| (1..=14).contains(count))
        .ok_or_else(|| {
            AppError::Validation("Sleep Night count must be from 1 through 14".to_owned())
        })?;
    Ok(Json(
        training::recent_sleep_nights(&state.noop_db, count).await?,
    ))
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

async fn exercise_history(
    State(state): State<crate::AppState>,
    Path((exercise_name, from_day, to_day)): Path<(String, String, String)>,
) -> Result<Json<ExerciseHistory>> {
    let from_day = parse_day(&from_day)?;
    let to_day = parse_day(&to_day)?;
    let days = from_day.days_until(to_day);
    if !(0..90).contains(&days) {
        return Err(AppError::Validation(
            "Exercise history must cover 1 to 90 calendar days, with from_day <= to_day"
                .to_string(),
        ));
    }
    Ok(Json(
        training::exercise_history(&state.db, &exercise_name, from_day, to_day).await?,
    ))
}

fn parse_day(day: &str) -> Result<CalendarDay> {
    CalendarDay::parse(day).ok_or_else(|| {
        AppError::Validation(format!(
            "Invalid day '{day}': must be a Europe/Copenhagen calendar date written YYYY-MM-DD"
        ))
    })
}
