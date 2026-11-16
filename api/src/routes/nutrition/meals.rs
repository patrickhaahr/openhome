use crate::AppState;
use crate::services::nutrition::food_database::FoodDatabase;
use crate::services::nutrition::meals;
use crate::services::nutrition::meals::{EditMeal, Meal, MealError, Meals, SaveMeal, Saved};
use axum::extract::rejection::JsonRejection;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::{Extension, Json};

fn meal_body<T>(body: Result<Json<T>, JsonRejection>) -> Result<T, MealError> {
    body.map(|Json(body)| body)
        .map_err(|rejection| MealError::Validation(rejection.body_text()))
}

pub(super) async fn list_meals(State(state): State<AppState>) -> Result<Json<Meals>, MealError> {
    Ok(Json(meals::meals(&state.db).await?))
}

pub(super) async fn save_meal(
    State(state): State<AppState>,
    Extension(foods): Extension<FoodDatabase>,
    request: Result<Json<SaveMeal>, JsonRejection>,
) -> Result<(StatusCode, Json<Saved>), MealError> {
    let saved = meals::save_meal(&state.db, &foods, meal_body(request)?).await?;
    let status = if saved.replayed {
        StatusCode::OK
    } else {
        StatusCode::CREATED
    };
    Ok((status, Json(saved)))
}

pub(super) async fn update_meal(
    State(state): State<AppState>,
    Extension(foods): Extension<FoodDatabase>,
    Path(id): Path<String>,
    request: Result<Json<EditMeal>, JsonRejection>,
) -> Result<Json<Meal>, MealError> {
    Ok(Json(
        meals::update_meal(&state.db, &foods, &id, meal_body(request)?).await?,
    ))
}

pub(super) async fn delete_meal(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, MealError> {
    meals::delete_meal(&state.db, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}
