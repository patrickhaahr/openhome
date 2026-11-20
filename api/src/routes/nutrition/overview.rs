use crate::AppState;
use crate::services::nutrition::day::NutritionDay;
use crate::services::nutrition::food_database::FoodDatabase;
use crate::services::nutrition::goals::{GoalError, NutritionGoal, SetGoal};
use crate::services::nutrition::meal_logs::MealLogError;
use crate::services::nutrition::summary::{NutritionSummary, SummaryQuery};
use crate::services::nutrition::{day, goals, meal_logs, summary};
use crate::services::training::calendar::CalendarDay;
use axum::extract::rejection::{JsonRejection, QueryRejection};
use axum::extract::{Path, Query, State};
use axum::{Extension, Json};

pub(super) async fn nutrition_day(
    State(state): State<AppState>,
    Extension(foods): Extension<FoodDatabase>,
    Path(day): Path<String>,
) -> Result<Json<NutritionDay>, MealLogError> {
    let day = meal_logs::parse_day(&day, "day")?;
    Ok(Json(
        day::nutrition_day(&state.db, &foods, day, today()).await?,
    ))
}

pub(super) async fn nutrition_summary(
    State(state): State<AppState>,
    query: Result<Query<SummaryQuery>, QueryRejection>,
) -> Result<Json<NutritionSummary>, MealLogError> {
    let Query(query) = query.map_err(|_| {
        MealLogError::Validation(
            "give from and to (YYYY-MM-DD) and aggregation day, week or month; no other \
             parameters are accepted"
                .into(),
        )
    })?;
    Ok(Json(
        summary::nutrition_summary(&state.db, &query, today()).await?,
    ))
}

fn today() -> CalendarDay {
    CalendarDay::of(chrono::Utc::now())
}

pub(super) async fn nutrition_goal(
    State(state): State<AppState>,
) -> Result<Json<NutritionGoal>, GoalError> {
    Ok(Json(goals::nutrition_goal(&state.db, today()).await?))
}

pub(super) async fn set_nutrition_goal(
    State(state): State<AppState>,
    request: Result<Json<SetGoal>, JsonRejection>,
) -> Result<Json<NutritionGoal>, GoalError> {
    let Json(request) =
        request.map_err(|rejection| GoalError::Validation(rejection.body_text()))?;
    Ok(Json(
        goals::set_nutrition_goal(&state.db, today(), request).await?,
    ))
}
