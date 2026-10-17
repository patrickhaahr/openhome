use axum::{
    Extension, Json, Router,
    extract::{Path, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, patch},
};
use serde_json::json;

use crate::{
    AppState,
    error::AppError,
    services::wishlist::{self, AddItem, CreateHouseholdUser, CreateWishlist, WishlistError},
    services::wishlist_metadata::{FetchError, MetadataPreview, MetadataRequest, MetadataService},
};

#[derive(Clone)]
struct DefaultList(Option<String>);

pub fn router(default_list_id: Option<String>) -> Router<AppState> {
    router_with_metadata(default_list_id, MetadataService::default())
}

pub(crate) fn router_with_metadata(
    default_list_id: Option<String>,
    metadata: MetadataService,
) -> Router<AppState> {
    Router::new()
        .route("/api/household-users", get(owners).post(create_owner))
        .route(
            "/api/household-users/{owner_id}",
            patch(rename_owner).delete(remove_owner),
        )
        .route(
            "/api/household-users/{owner_id}/wishlists/{id}",
            patch(rename_list).delete(remove_list),
        )
        .route(
            "/api/household-users/{owner_id}/wishlists/{list_id}/items/{id}",
            get(item).patch(edit_item).delete(remove_item),
        )
        .route(
            "/api/household-users/{owner_id}/wishlists/{list_id}/items/{id}/image",
            get(item_image),
        )
        .route("/api/wishlists", get(lists).post(create_list))
        .route("/api/wishlists/{id}/items", get(items))
        .route("/api/wishlist/items", axum::routing::post(add_item))
        .route(
            "/api/wishlist/url-metadata",
            axum::routing::post(url_metadata),
        )
        .layer(Extension(metadata))
        .layer(Extension(DefaultList(default_list_id)))
}

async fn url_metadata(
    State(state): State<AppState>,
    Extension(metadata): Extension<MetadataService>,
    Json(request): Json<MetadataRequest>,
) -> Result<Json<MetadataPreview>, WishlistError> {
    wishlist::require_list_owner(&state.db, &request.owner_id, &request.list_id).await?;
    Ok(Json(metadata.preview(&request.url).await?))
}

/// The stored picture of one item, fetched by Axum so the browser never contacts the shop.
/// Only the item's own `image_url` is fetched; the address is never logged or echoed.
async fn item_image(
    State(state): State<AppState>,
    Extension(metadata): Extension<MetadataService>,
    Path((owner_id, list_id, id)): Path<(String, String, String)>,
) -> Result<Response, WishlistError> {
    let item = wishlist::item(&state.db, &owner_id, &list_id, &id).await?;
    let Some(image_url) = item.image_url else {
        return Err(WishlistError::NotFound(
            "This Wishlist Item has no picture".into(),
        ));
    };
    let image = match metadata.image(&image_url).await {
        Ok(image) => image,
        Err(FetchError::Blocked) => {
            return Ok(AppError::Unprocessable(
                "The picture address is not a public HTTP(S) destination".into(),
            )
            .into_response());
        }
        Err(FetchError::Unavailable) => {
            return Ok(AppError::ServiceUnavailable(
                "The picture could not be loaded as a raster image within limits".into(),
            )
            .into_response());
        }
    };
    Ok((
        [
            (header::CONTENT_TYPE, image.content_type),
            // Private: the picture belongs to a household list behind the API key.
            (header::CACHE_CONTROL, "private, max-age=3600"),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
            (
                header::CONTENT_SECURITY_POLICY,
                "default-src 'none'; sandbox",
            ),
        ],
        image.body,
    )
        .into_response())
}

async fn rename_owner(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(request): Json<wishlist::Rename>,
) -> Result<Json<wishlist::HouseholdUser>, WishlistError> {
    Ok(Json(wishlist::rename_owner(&state.db, &id, request).await?))
}

async fn remove_owner(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, WishlistError> {
    wishlist::remove_owner(&state.db, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn rename_list(
    State(state): State<AppState>,
    Path((owner_id, id)): Path<(String, String)>,
    Json(request): Json<wishlist::Rename>,
) -> Result<Json<wishlist::Wishlist>, WishlistError> {
    Ok(Json(
        wishlist::rename_list(&state.db, &owner_id, &id, request).await?,
    ))
}

async fn remove_list(
    State(state): State<AppState>,
    Path((owner_id, id)): Path<(String, String)>,
) -> Result<StatusCode, WishlistError> {
    wishlist::remove_list(&state.db, &owner_id, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn item(
    State(state): State<AppState>,
    Path((owner_id, list_id, id)): Path<(String, String, String)>,
) -> Result<Json<wishlist::WishlistItem>, WishlistError> {
    Ok(Json(
        wishlist::item(&state.db, &owner_id, &list_id, &id).await?,
    ))
}

async fn edit_item(
    State(state): State<AppState>,
    Path((owner_id, list_id, id)): Path<(String, String, String)>,
    Json(request): Json<wishlist::EditItem>,
) -> Result<Json<wishlist::WishlistItem>, WishlistError> {
    Ok(Json(
        wishlist::edit_item(&state.db, &owner_id, &list_id, &id, request).await?,
    ))
}

async fn remove_item(
    State(state): State<AppState>,
    Path((owner_id, list_id, id)): Path<(String, String, String)>,
) -> Result<StatusCode, WishlistError> {
    wishlist::remove_item(&state.db, &owner_id, &list_id, &id).await?;
    Ok(StatusCode::NO_CONTENT)
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
