use crate::AppState;
use crate::services::nutrition::food_database::FoodDatabase;
use crate::services::nutrition::meal_logs;
use crate::services::nutrition::meal_logs::{
    EditMealLog, LogMeal, Logged, MealLog, MealLogError, MealLogs, MealLogsQuery,
};
use crate::services::nutrition::meal_photos::{MealPhotos, PhotoError, Upload, Variant};
use axum::extract::multipart::{MultipartError, MultipartRejection};
use axum::extract::rejection::{JsonRejection, QueryRejection};
use axum::extract::{Multipart, Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};

fn body<T>(body: Result<Json<T>, JsonRejection>) -> Result<T, MealLogError> {
    body.map(|Json(body)| body).map_err(|rejection| {
        if rejection.status() == StatusCode::PAYLOAD_TOO_LARGE {
            PhotoError::TooLarge.into()
        } else {
            MealLogError::Validation(rejection.body_text())
        }
    })
}

pub(super) async fn log_meal(
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

pub(super) async fn list_meal_logs(
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

pub(super) async fn update_meal_log(
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

pub(super) async fn upload_photo(
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

pub(super) async fn delete_meal_log(
    State(state): State<AppState>,
    Extension(photos): Extension<MealPhotos>,
    Path(id): Path<String>,
) -> Result<StatusCode, MealLogError> {
    meal_logs::delete_meal_log(&state.db, &photos, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn photo(
    Extension(photos): Extension<MealPhotos>,
    Path(photo_id): Path<String>,
) -> Result<Response, MealLogError> {
    media(&photos, &photo_id, Variant::Photo).await
}

pub(super) async fn thumbnail(
    Extension(photos): Extension<MealPhotos>,
    Path(photo_id): Path<String>,
) -> Result<Response, MealLogError> {
    media(&photos, &photo_id, Variant::Thumbnail).await
}

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
