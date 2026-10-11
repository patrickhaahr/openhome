use axum::{
    Extension, Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};
use serde_json::json;

use crate::{
    AppState,
    error::AppError,
    services::wishlist::{self, AddItem, CreateHouseholdUser, CreateWishlist, WishlistError},
};

#[derive(Clone)]
struct DefaultList(Option<String>);

pub fn router(default_list_id: Option<String>) -> Router<AppState> {
    Router::new()
        .route("/api/household-users", get(owners).post(create_owner))
        .route("/api/wishlists", get(lists).post(create_list))
        .route("/api/wishlists/{id}/items", get(items))
        .route("/api/wishlist/items", axum::routing::post(add_item))
        .layer(Extension(DefaultList(default_list_id)))
}

impl IntoResponse for WishlistError {
    fn into_response(self) -> Response {
        match self {
            Self::Validation(message) => AppError::Validation(message).into_response(),
            Self::NotFound(message) => AppError::NotFound(message).into_response(),
            Self::Conflict(message) => AppError::Conflict(message).into_response(),
            Self::DestinationRequired { choices } => (
                StatusCode::CONFLICT,
                Json(json!({
                    "error": "Choose a Wishlist, or create a named list for the intended Household User first",
                    "status": 409,
                    "code": "wishlist_destination_required",
                    "question": "Which household owner's wishlist should receive this item?",
                    "choices": choices,
                })),
            ).into_response(),
            Self::Database(sqlx::Error::Database(error)) if error.is_unique_violation() =>
                AppError::Conflict("This name already exists for the household or list owner".into()).into_response(),
            Self::Database(error) => AppError::Internal(error.into()).into_response(),
        }
    }
}

async fn owners(
    State(state): State<AppState>,
) -> Result<Json<Vec<wishlist::HouseholdUser>>, WishlistError> {
    Ok(Json(wishlist::owners(&state.db).await?))
}

async fn create_owner(
    State(state): State<AppState>,
    Json(request): Json<CreateHouseholdUser>,
) -> Result<(StatusCode, Json<wishlist::HouseholdUser>), WishlistError> {
    Ok((
        StatusCode::CREATED,
        Json(wishlist::create_owner(&state.db, request).await?),
    ))
}

async fn lists(
    State(state): State<AppState>,
) -> Result<Json<Vec<wishlist::Wishlist>>, WishlistError> {
    Ok(Json(wishlist::lists(&state.db).await?))
}

async fn create_list(
    State(state): State<AppState>,
    Json(request): Json<CreateWishlist>,
) -> Result<(StatusCode, Json<wishlist::Wishlist>), WishlistError> {
    Ok((
        StatusCode::CREATED,
        Json(wishlist::create_list(&state.db, request).await?),
    ))
}

async fn items(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Vec<wishlist::WishlistItem>>, WishlistError> {
    Ok(Json(wishlist::items(&state.db, &id).await?))
}

async fn add_item(
    State(state): State<AppState>,
    Extension(DefaultList(default_list)): Extension<DefaultList>,
    Json(request): Json<AddItem>,
) -> Result<(StatusCode, Json<wishlist::Addition>), WishlistError> {
    let result = wishlist::add_item(&state.db, request, default_list.as_deref()).await?;
    let status = if result.replayed {
        StatusCode::OK
    } else {
        StatusCode::CREATED
    };
    Ok((status, Json(result)))
}
