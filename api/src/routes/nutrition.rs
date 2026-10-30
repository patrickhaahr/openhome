//! Nutrition endpoints under `/api/nutrition`.

use axum::{
    Json, Router,
    extract::{Path, Query, State, rejection::QueryRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};
use serde::Deserialize;
use serde_json::json;

use crate::{
    error::AppError,
    services::nutrition::food_database::{Food, FoodDatabase, FoodError, FoodSearch, Unavailable},
};

pub fn router(foods: FoodDatabase) -> Router<crate::AppState> {
    Router::new()
        .route("/api/nutrition/foods", get(search_foods))
        .route("/api/nutrition/foods/{food_id}", get(food))
        .with_state(foods)
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
