//! Read-only training context for API clients such as the Hermes MCP adapter. The rules live in
//! `services::training`; handlers only validate input.

use axum::{
    Json, Router,
    extract::{Path, State},
    routing::get,
};
use chrono::{DateTime, Utc};

use crate::error::{AppError, Result};
use crate::services::training::{
    self, CalendarDay, ExerciseHistory, Metric, MetricTrend, NoopWorkoutDetail, RecentSleepNights,
    RecoveryDay, TrainingContext, WorkoutsOnDay,
};

pub fn router() -> Router<crate::AppState> {
    Router::new()
        .route("/api/training/days/{day}/recovery", get(recovery_day))
        .route("/api/training/days/{day}/workouts", get(workouts_on_day))
        .route(
            "/api/training/noop-workouts/{source}/{start}",
            get(noop_workout_detail),
        )
        .route(
            "/api/training/noop-workouts/{source}/{start}/{sport}",
            get(noop_workout_detail_of_sport),
        )
        .route(
            "/api/training/sleep/recent/{count}",
            get(recent_sleep_nights),
        )
        .route(
            "/api/training/exercises/{exercise_name}/history/{from_day}/{to_day}",
            get(exercise_history),
        )
        .route(
            "/api/training/trends/{metric}/{from_day}/{to_day}",
            get(metric_trend),
        )
        .route(
            "/api/training/context/{from_day}/{to_day}",
            get(training_context),
        )
}

async fn training_context(
    State(state): State<crate::AppState>,
    Path((from_day, to_day)): Path<(String, String)>,
) -> Result<Json<TrainingContext>> {
    let (from_day, to_day) = parse_range("Training Context", &from_day, &to_day)?;
    Ok(Json(
        training::training_context(&state.db, &state.noop_db, from_day, to_day).await?,
    ))
}

async fn metric_trend(
    State(state): State<crate::AppState>,
    Path((metric, from_day, to_day)): Path<(String, String, String)>,
) -> Result<Json<MetricTrend>> {
    let metric = Metric::parse(&metric).ok_or_else(|| {
        AppError::Validation(format!(
            "Unknown trend metric '{metric}'; expected body_weight, sleep_duration, resting_heart_rate or hrv"
        ))
    })?;
    let (from_day, to_day) = parse_range("Metric trend", &from_day, &to_day)?;
    Ok(Json(
        training::metric_trend(&state.db, &state.noop_db, metric, from_day, to_day).await?,
    ))
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

async fn noop_workout_detail(
    State(state): State<crate::AppState>,
    Path((source, start)): Path<(String, String)>,
) -> Result<Json<NoopWorkoutDetail>> {
    read_noop_workout_detail(&state, &source, &start, None).await
}

async fn noop_workout_detail_of_sport(
    State(state): State<crate::AppState>,
    Path((source, start, sport)): Path<(String, String, String)>,
) -> Result<Json<NoopWorkoutDetail>> {
    read_noop_workout_detail(&state, &source, &start, Some(&sport)).await
}

/// Validates the NOOP Workout's listed `source`, `start` and optional `sport`, then reads it.
async fn read_noop_workout_detail(
    state: &crate::AppState,
    source: &str,
    start: &str,
    sport: Option<&str>,
) -> Result<Json<NoopWorkoutDetail>> {
    let malformed = |value: &str| value.is_empty() || value.chars().any(char::is_control);
    if malformed(source) {
        return Err(AppError::Validation(format!(
            "Invalid source {source:?}: must be a NOOP device namespace as workouts_on_day lists it"
        )));
    }
    if let Some(sport) = sport.filter(|sport| malformed(sport)) {
        return Err(AppError::Validation(format!(
            "Invalid sport {sport:?}: must be a NOOP Workout sport as workouts_on_day lists it"
        )));
    }
    let start = DateTime::parse_from_rfc3339(start)
        .map_err(|_| {
            AppError::Validation(format!(
                "Invalid start '{start}': must be an RFC 3339 timestamp with an offset, as \
                 workouts_on_day lists it"
            ))
        })?
        .with_timezone(&Utc);
    Ok(Json(
        training::noop_workout_detail(&state.noop_db, source, start, sport).await?,
    ))
}

async fn exercise_history(
    State(state): State<crate::AppState>,
    Path((exercise_name, from_day, to_day)): Path<(String, String, String)>,
) -> Result<Json<ExerciseHistory>> {
    let (from_day, to_day) = parse_range("Exercise history", &from_day, &to_day)?;
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

/// An inclusive range of 1 through 90 calendar days, `read` naming the read in the error.
fn parse_range(read: &str, from_day: &str, to_day: &str) -> Result<(CalendarDay, CalendarDay)> {
    let from_day = parse_day(from_day)?;
    let to_day = parse_day(to_day)?;
    if !(0..90).contains(&from_day.days_until(to_day)) {
        return Err(AppError::Validation(format!(
            "{read} must cover 1 to 90 calendar days, with from_day <= to_day"
        )));
    }
    Ok((from_day, to_day))
}
