use crate::error::AppError;
use crate::services::nutrition::food_database::{FoodError, Unavailable};
use crate::services::nutrition::goals::GoalError;
use crate::services::nutrition::meal_logs::MealLogError;
use crate::services::nutrition::meal_photos::PhotoError;
use crate::services::nutrition::meals::MealError;
use crate::services::nutrition::open_food_facts::ProviderError;
use crate::services::nutrition::products::ProductError;
use crate::services::nutrition::supplements::SupplementError;
use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

impl IntoResponse for GoalError {
    fn into_response(self) -> Response {
        match self {
            Self::Validation(message) => AppError::Validation(message).into_response(),
            Self::Database(error) => AppError::Internal(error.into()).into_response(),
        }
    }
}

impl IntoResponse for MealLogError {
    fn into_response(self) -> Response {
        match self {
            Self::Validation(message) => AppError::Validation(message).into_response(),
            Self::NotFound(message) => AppError::NotFound(message).into_response(),
            Self::Conflict(message) => AppError::Conflict(message).into_response(),
            Self::ProductNeedsRelink(message) => {
                let status = StatusCode::CONFLICT;
                (
                    status,
                    Json(json!({
                        "error": message,
                        "status": status.as_u16(),
                        "code": "product_needs_relink",
                    })),
                )
                    .into_response()
            }
            Self::Food(error) => error.into_response(),
            Self::Photo(error) => error.into_response(),
            Self::Meal(error) => error.into_response(),
            Self::Supplement(error) => error.into_response(),
            Self::Database(error) => AppError::Internal(error.into()).into_response(),
        }
    }
}

impl IntoResponse for MealError {
    fn into_response(self) -> Response {
        match self {
            Self::Validation(message) => AppError::Validation(message).into_response(),
            Self::NotFound(message) => AppError::NotFound(message).into_response(),
            Self::Conflict(message) => AppError::Conflict(message).into_response(),
            Self::NeedsUpdate(message) => {
                let status = StatusCode::CONFLICT;
                (
                    status,
                    Json(json!({
                        "error": message,
                        "status": status.as_u16(),
                        "code": "meal_needs_update",
                    })),
                )
                    .into_response()
            }
            Self::Food(error) => error.into_response(),
            Self::Supplement(error) => error.into_response(),
            Self::Database(error) => AppError::Internal(error.into()).into_response(),
        }
    }
}

impl IntoResponse for SupplementError {
    fn into_response(self) -> Response {
        match self {
            Self::Validation(message) => AppError::Validation(message).into_response(),
            Self::NotFound(message) => AppError::NotFound(message).into_response(),
            Self::Conflict(message) => AppError::Conflict(message).into_response(),
            Self::Database(error) => AppError::Internal(error.into()).into_response(),
        }
    }
}

impl IntoResponse for ProductError {
    fn into_response(self) -> Response {
        match self {
            Self::Validation(message) => AppError::Validation(message).into_response(),
            Self::NotFound(_) => {
                let status = StatusCode::NOT_FOUND;
                (
                    status,
                    Json(json!({
                        "error": format!(
                            "{self}; log the closest Food by food_id instead, with estimates for \
                             values the package label gives"
                        ),
                        "status": status.as_u16(),
                        "code": "product_not_found",
                    })),
                )
                    .into_response()
            }
            Self::Conflict(message) => AppError::Conflict(message).into_response(),
            Self::Provider(ProviderError::Unavailable) => {
                let status = StatusCode::SERVICE_UNAVAILABLE;
                (
                    status,
                    Json(json!({
                        "error": "Open Food Facts did not answer; try again in a minute, or log \
                                  the closest Food by food_id",
                        "status": status.as_u16(),
                        "code": "open_food_facts_unavailable",
                    })),
                )
                    .into_response()
            }
            Self::Food(error) => error.into_response(),
            Self::Database(error) => AppError::Internal(error.into()).into_response(),
        }
    }
}

impl IntoResponse for PhotoError {
    fn into_response(self) -> Response {
        match self {
            Self::Undecodable => AppError::Validation(self.to_string()).into_response(),
            Self::TooLarge => {
                let status = StatusCode::PAYLOAD_TOO_LARGE;
                (
                    status,
                    Json(json!({"error": self.to_string(), "status": status.as_u16()})),
                )
                    .into_response()
            }
            Self::Encode | Self::Io(_) => AppError::Internal(self.into()).into_response(),
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
