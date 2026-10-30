//! Nutrition endpoints under `/api/nutrition`.

use axum::{
    Extension, Json, Router,
    extract::{
        Path, Query, State,
        rejection::{JsonRejection, QueryRejection},
    },
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};
use serde::Deserialize;
use serde_json::json;

use crate::{
    AppState,
    error::AppError,
    services::nutrition::{
        day::{self, NutritionDay},
        food_database::{Food, FoodDatabase, FoodError, FoodSearch, Unavailable},
        meal_logs::{
            self, EditMealLog, LogMeal, Logged, MealLog, MealLogError, MealLogs, MealLogsQuery,
        },
    },
};

pub fn router(foods: FoodDatabase) -> Router<AppState> {
    // Meal Logs live in app.db and snapshot Foods from food.db, so they need both.
    let meal_logs = Router::new()
        .route(
            "/api/nutrition/meal-logs",
            get(list_meal_logs).post(log_meal),
        )
        .route(
            "/api/nutrition/meal-logs/{id}",
            axum::routing::patch(update_meal_log).delete(delete_meal_log),
        )
        .route("/api/nutrition/days/{day}", get(nutrition_day))
        .layer(Extension(foods.clone()));
    Router::new()
        .route("/api/nutrition/foods", get(search_foods))
        .route("/api/nutrition/foods/{food_id}", get(food))
        .with_state(foods)
        .merge(meal_logs)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SearchQuery {
    query: String,
    limit: Option<usize>,
}

async fn search_foods(
    State(foods): State<FoodDatabase>,
    query: Result<Query<SearchQuery>, QueryRejection>,
) -> Result<Json<FoodSearch>, FoodError> {
    let Query(SearchQuery { query, limit }) = query.map_err(|_| {
        FoodError::Validation(
            "query is required; limit must be a whole number; no other parameters are accepted"
                .into(),
        )
    })?;
    Ok(Json(foods.search(&query, limit).await?))
}

async fn food(
    State(foods): State<FoodDatabase>,
    Path(food_id): Path<String>,
) -> Result<Json<Food>, FoodError> {
    let food_id = food_id
        .parse::<i64>()
        .ok()
        .filter(|id| *id > 0)
        .ok_or_else(|| FoodError::Validation("food_id must be a positive whole number".into()))?;
    Ok(Json(foods.food(food_id).await?))
}

fn body<T>(body: Result<Json<T>, JsonRejection>) -> Result<T, MealLogError> {
    body.map(|Json(body)| body)
        .map_err(|rejection| MealLogError::Validation(rejection.body_text()))
}

async fn log_meal(
    State(state): State<AppState>,
    Extension(foods): Extension<FoodDatabase>,
    request: Result<Json<LogMeal>, JsonRejection>,
) -> Result<(StatusCode, Json<Logged>), MealLogError> {
    let logged = meal_logs::log_meal(&state.db, &foods, body(request)?).await?;
    let status = if logged.replayed {
        StatusCode::OK
    } else {
        StatusCode::CREATED
    };
    Ok((status, Json(logged)))
}

async fn list_meal_logs(
    State(state): State<AppState>,
    query: Result<Query<MealLogsQuery>, QueryRejection>,
) -> Result<Json<MealLogs>, MealLogError> {
    let Query(query) = query.map_err(|_| {
        MealLogError::Validation(
            "give either day, or both from and to (YYYY-MM-DD); no other parameters are accepted"
                .into(),
        )
    })?;
    Ok(Json(meal_logs::meal_logs(&state.db, &query).await?))
}

async fn update_meal_log(
    State(state): State<AppState>,
    Extension(foods): Extension<FoodDatabase>,
    Path(id): Path<String>,
    request: Result<Json<EditMealLog>, JsonRejection>,
) -> Result<Json<MealLog>, MealLogError> {
    Ok(Json(
        meal_logs::update_meal_log(&state.db, &foods, &id, body(request)?).await?,
    ))
}

async fn delete_meal_log(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, MealLogError> {
    meal_logs::delete_meal_log(&state.db, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn nutrition_day(
    State(state): State<AppState>,
    Extension(foods): Extension<FoodDatabase>,
    Path(day): Path<String>,
) -> Result<Json<NutritionDay>, MealLogError> {
    let day = meal_logs::parse_day(&day, "day")?;
    Ok(Json(day::nutrition_day(&state.db, &foods, day).await?))
}

impl IntoResponse for MealLogError {
    fn into_response(self) -> Response {
        match self {
            Self::Validation(message) => AppError::Validation(message).into_response(),
            Self::NotFound(message) => AppError::NotFound(message).into_response(),
            Self::Conflict(message) => AppError::Conflict(message).into_response(),
            Self::Food(error) => error.into_response(),
            Self::Database(error) => AppError::Internal(error.into()).into_response(),
        }
    }
}

impl IntoResponse for FoodError {
    fn into_response(self) -> Response {
        match self {
            Self::Unavailable(state) => {
                let error = match state {
                    Unavailable::Importing => {
                        "The food database is being imported; try again in a few minutes"
                    }
                    Unavailable::Failed => {
                        "The food database could not be imported; it is retried when the API restarts"
                    }
                };
                (
                    StatusCode::SERVICE_UNAVAILABLE,
                    Json(json!({
                        "error": error,
                        "status": 503,
                        "code": "food_database_unavailable",
                        "state": state,
                    })),
                )
                    .into_response()
            }
            Self::Validation(message) => AppError::Validation(message).into_response(),
            Self::NotFound(_) => AppError::NotFound(self.to_string()).into_response(),
            Self::Database(error) => AppError::Internal(error.into()).into_response(),
        }
    }
}
