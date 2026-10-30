//! Nutrition endpoints under `/api/nutrition`.

use axum::{
    Extension, Json, Router,
    extract::{
        DefaultBodyLimit, Multipart, Path, Query, State,
        multipart::{MultipartError, MultipartRejection},
        rejection::{JsonRejection, QueryRejection},
    },
    handler::Handler,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, put},
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
        meal_photos::{MealPhotos, PHOTO_BODY_LIMIT, PhotoError, Upload, Variant},
        nova::{self, ClassifiedFood, ClassifyFood},
        open_food_facts::{OpenFoodFacts, ProviderError},
        products::{self, LinkProduct, Linked, ProductError, ProductLookup},
    },
};

pub fn router(foods: FoodDatabase, photos: MealPhotos, labels: OpenFoodFacts) -> Router<AppState> {
    // Only the routes that carry a photo raise Axum's default body limit.
    let photo_limit = DefaultBodyLimit::max(PHOTO_BODY_LIMIT);
    // Foods come from food.db with their NOVA Classes from app.db; Meal Logs and Products live
    // in app.db and take values from food.db and Open Food Facts; Meal Logs keep photos in the
    // data directory.
    Router::new()
        .route("/api/nutrition/foods", get(search_foods))
        .route("/api/nutrition/foods/{food_id}", get(food))
        .route(
            "/api/nutrition/foods/{food_id}/nova-class",
            put(classify_food),
        )
        .route("/api/nutrition/products", axum::routing::post(link_product))
        .route("/api/nutrition/products/{barcode}", get(lookup_product))
        .route(
            "/api/nutrition/meal-logs",
            get(list_meal_logs).post(log_meal.layer(photo_limit)),
        )
        .route(
            "/api/nutrition/meal-logs/{id}",
            axum::routing::patch(update_meal_log.layer(photo_limit)).delete(delete_meal_log),
        )
        .route(
            "/api/nutrition/meal-logs/{id}/photo",
            put(upload_photo.layer(photo_limit)),
        )
        .route("/api/nutrition/meal-photos/{photo_id}", get(photo))
        .route(
            "/api/nutrition/meal-photos/{photo_id}/thumbnail",
            get(thumbnail),
        )
        .route("/api/nutrition/days/{day}", get(nutrition_day))
        .layer(Extension(foods))
        .layer(Extension(photos))
        .layer(Extension(labels))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SearchQuery {
    query: String,
    limit: Option<usize>,
}

async fn search_foods(
    State(state): State<AppState>,
    Extension(foods): Extension<FoodDatabase>,
    query: Result<Query<SearchQuery>, QueryRejection>,
) -> Result<Json<FoodSearch>, FoodError> {
    let Query(SearchQuery { query, limit }) = query.map_err(|_| {
        FoodError::Validation(
            "query is required; limit must be a whole number; no other parameters are accepted"
                .into(),
        )
    })?;
    Ok(Json(
        nova::search_foods(&state.db, &foods, &query, limit).await?,
    ))
}

fn food_id(text: &str) -> Result<i64, FoodError> {
    text.parse::<i64>()
        .ok()
        .filter(|id| *id > 0)
        .ok_or_else(|| FoodError::Validation("food_id must be a positive whole number".into()))
}

async fn food(
    State(state): State<AppState>,
    Extension(foods): Extension<FoodDatabase>,
    Path(id): Path<String>,
) -> Result<Json<Food>, FoodError> {
    Ok(Json(nova::food(&state.db, &foods, food_id(&id)?).await?))
}

/// Sets or changes a Food's NOVA Class.
async fn classify_food(
    State(state): State<AppState>,
    Extension(foods): Extension<FoodDatabase>,
    Path(id): Path<String>,
    request: Result<Json<ClassifyFood>, JsonRejection>,
) -> Result<Json<ClassifiedFood>, FoodError> {
    let food_id = food_id(&id)?;
    let Json(request) =
        request.map_err(|rejection| FoodError::Validation(rejection.body_text()))?;
    Ok(Json(
        nova::classify_food(&state.db, &foods, food_id, request).await?,
    ))
}

async fn lookup_product(
    State(state): State<AppState>,
    Extension(foods): Extension<FoodDatabase>,
    Extension(labels): Extension<OpenFoodFacts>,
    Path(barcode): Path<String>,
) -> Result<Json<ProductLookup>, ProductError> {
    Ok(Json(
        products::lookup(&state.db, &foods, &labels, &barcode).await?,
    ))
}

async fn link_product(
    State(state): State<AppState>,
    Extension(foods): Extension<FoodDatabase>,
    Extension(labels): Extension<OpenFoodFacts>,
    request: Result<Json<LinkProduct>, JsonRejection>,
) -> Result<(StatusCode, Json<Linked>), ProductError> {
    let Json(request) =
        request.map_err(|rejection| ProductError::Validation(rejection.body_text()))?;
    let linked = products::link(&state.db, &foods, &labels, request).await?;
    let status = if linked.created {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };
    Ok((status, Json(linked)))
}

fn body<T>(body: Result<Json<T>, JsonRejection>) -> Result<T, MealLogError> {
    body.map(|Json(body)| body).map_err(|rejection| {
        if rejection.status() == StatusCode::PAYLOAD_TOO_LARGE {
            PhotoError::TooLarge.into()
        } else {
            MealLogError::Validation(rejection.body_text())
        }
    })
}

async fn log_meal(
    State(state): State<AppState>,
    Extension(foods): Extension<FoodDatabase>,
    Extension(photos): Extension<MealPhotos>,
    request: Result<Json<LogMeal>, JsonRejection>,
) -> Result<(StatusCode, Json<Logged>), MealLogError> {
    let logged = meal_logs::log_meal(&state.db, &foods, &photos, body(request)?).await?;
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
    Extension(photos): Extension<MealPhotos>,
    Path(id): Path<String>,
    request: Result<Json<EditMealLog>, JsonRejection>,
) -> Result<Json<MealLog>, MealLogError> {
    Ok(Json(
        meal_logs::update_meal_log(&state.db, &foods, &photos, &id, body(request)?).await?,
    ))
}

fn multipart_error(error: &MultipartError) -> MealLogError {
    if error.status() == StatusCode::PAYLOAD_TOO_LARGE {
        PhotoError::TooLarge.into()
    } else {
        MealLogError::Validation(
            "send the photo as multipart/form-data with a JPEG or PNG file field named photo"
                .into(),
        )
    }
}

/// Attaches or replaces a Meal Log's photo from the multipart file field `photo`, for apps that
/// upload files; other fields are ignored.
async fn upload_photo(
    State(state): State<AppState>,
    Extension(photos): Extension<MealPhotos>,
    Path(id): Path<String>,
    multipart: Result<Multipart, MultipartRejection>,
) -> Result<Json<MealLog>, MealLogError> {
    let mut multipart = multipart.map_err(|_| {
        MealLogError::Validation(
            "send the photo as multipart/form-data with a JPEG or PNG file field named photo"
                .into(),
        )
    })?;
    let mut upload = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|error| multipart_error(&error))?
    {
        if field.name() == Some("photo") {
            let bytes = field
                .bytes()
                .await
                .map_err(|error| multipart_error(&error))?;
            upload = Some(Upload::from_bytes(bytes.to_vec())?);
            break;
        }
    }
    let upload = upload.ok_or_else(|| {
        MealLogError::Validation("the multipart body has no file field named photo".into())
    })?;
    Ok(Json(
        meal_logs::replace_photo(&state.db, &photos, &id, upload).await?,
    ))
}

async fn delete_meal_log(
    State(state): State<AppState>,
    Extension(photos): Extension<MealPhotos>,
    Path(id): Path<String>,
) -> Result<StatusCode, MealLogError> {
    meal_logs::delete_meal_log(&state.db, &photos, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn photo(
    Extension(photos): Extension<MealPhotos>,
    Path(photo_id): Path<String>,
) -> Result<Response, MealLogError> {
    media(&photos, &photo_id, Variant::Photo).await
}

async fn thumbnail(
    Extension(photos): Extension<MealPhotos>,
    Path(photo_id): Path<String>,
) -> Result<Response, MealLogError> {
    media(&photos, &photo_id, Variant::Thumbnail).await
}

/// A stored AVIF file. A photo ID always names the same image, so clients may cache it for good.
async fn media(
    photos: &MealPhotos,
    photo_id: &str,
    variant: Variant,
) -> Result<Response, MealLogError> {
    let bytes = photos
        .read(photo_id, variant)
        .await?
        .ok_or_else(|| MealLogError::NotFound("Meal photo not found".into()))?;
    Ok((
        [
            (header::CONTENT_TYPE, "image/avif"),
            (
                header::CACHE_CONTROL,
                "private, max-age=31536000, immutable",
            ),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
        ],
        bytes,
    )
        .into_response())
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
            Self::Photo(error) => error.into_response(),
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
