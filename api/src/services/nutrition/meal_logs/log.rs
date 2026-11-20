use super::{
    Contents, KEY_NUTRIENTS, LogMeal, Logged, MealLog, MealLogError, PreparedPortion, Result,
    Selection, StoredPhoto, ValidPortion, Values, contents, eaten_at, find, insert_portions,
    load_on, not_found, note, prepare, prepare_doses, request_id, utc_text,
};
use crate::services::nutrition::food_database::FoodDatabase;
use crate::services::nutrition::meal_photos::{MealPhotos, Upload};
use crate::services::nutrition::meals;
use crate::services::nutrition::supplements::{self, StoredDose};
use crate::services::training::calendar::CalendarDay;
use chrono::Utc;
use sqlx::SqlitePool;
use std::collections::BTreeMap;
use uuid::Uuid;

async fn replay(db: &SqlitePool, request_id: &str, request: &str) -> Result<Option<Logged>> {
    let Some(receipt) = sqlx::query!(
        "SELECT meal_log_id, request, result FROM meal_log_receipts WHERE request_id = ?",
        request_id
    )
    .fetch_optional(db)
    .await?
    else {
        return Ok(None);
    };
    if receipt.request != request {
        return Err(MealLogError::Conflict(
            "request_id was already used for a different Meal Log; use a new ID for a new meal"
                .into(),
        ));
    }
    find(db, &receipt.meal_log_id).await?.ok_or_else(|| {
        MealLogError::Conflict(
            "The retried Meal Log was deleted; use a new request_id to log it again".into(),
        )
    })?;
    let result = receipt.result.ok_or_else(|| {
        MealLogError::Conflict(
            "This legacy receipt has no original result; read the current meal_log before retrying"
                .into(),
        )
    })?;
    let meal_log =
        serde_json::from_str(&result).map_err(|error| sqlx::Error::Decode(Box::new(error)))?;
    Ok(Some(Logged {
        meal_log,
        replayed: true,
    }))
}

#[derive(Default)]
pub(super) struct PhotoColumns {
    pub(super) id: Option<String>,
    pub(super) width: Option<i64>,
    pub(super) height: Option<i64>,
    pub(super) thumbnail_width: Option<i64>,
    pub(super) thumbnail_height: Option<i64>,
}

impl PhotoColumns {
    pub(super) fn of(photo: Option<&StoredPhoto>) -> Self {
        photo.map_or_else(Self::default, |photo| Self {
            id: Some(photo.id.clone()),
            width: Some(photo.size.width.into()),
            height: Some(photo.size.height.into()),
            thumbnail_width: Some(photo.thumbnail_size.width.into()),
            thumbnail_height: Some(photo.thumbnail_size.height.into()),
        })
    }
}

pub(super) async fn store_photo(photos: &MealPhotos, upload: Upload) -> Result<StoredPhoto> {
    let encoded = photos.encode(upload).await?;
    let id = photos.store(&encoded).await?;
    Ok(StoredPhoto {
        id,
        size: encoded.size,
        thumbnail_size: encoded.thumbnail_size,
    })
}

pub async fn log_meal(
    db: &SqlitePool,
    foods: &FoodDatabase,
    photos: &MealPhotos,
    request: LogMeal,
) -> Result<Logged> {
    let request_id = request_id(&request.request_id)?;
    let at = eaten_at(&request.eaten_at, Utc::now())?;
    let note = note(request.note)?;
    let contents = contents(
        request.portions,
        request.supplement_doses,
        request.meal,
        request.overrides,
    )?;
    let upload = request
        .photo
        .as_deref()
        .map(Upload::from_base64)
        .transpose()?;
    let stored_at = utc_text(at);
    let normalized = serde_json::to_string(&contents.normalized(
        &stored_at,
        note.as_deref(),
        upload.as_ref().map(Upload::sha256),
    ))
    .map_err(|error| sqlx::Error::Encode(Box::new(error)))?;
    if let Some(logged) = replay(db, &request_id, &normalized).await? {
        return Ok(logged);
    }
    let (requests, doses, meal_name) = match contents {
        Contents::Items { portions, doses } => (portions, doses, None),
        Contents::Meal {
            name,
            overrides,
            dose_overrides,
        } => {
            let meal = meals::resolve(db, foods, &name, &overrides, &dose_overrides).await?;
            let requests = meal
                .items
                .into_iter()
                .map(|(item, grams)| ValidPortion {
                    item,
                    grams,
                    estimates: BTreeMap::new(),
                })
                .collect();
            (requests, meal.doses, Some(meal.name))
        }
    };
    let prepared = prepare(db, foods, &requests).await?;
    let prepared_doses = prepare_doses(db, &doses).await?;
    let photo = match upload {
        Some(upload) => Some(store_photo(photos, upload).await?),
        None => None,
    };

    let id = Uuid::new_v4().to_string();
    let row = NewLog {
        id: &id,
        request_id: &request_id,
        normalized: &normalized,
        eaten_at: &stored_at,
        day: CalendarDay::of(at).to_string(),
        meal_name: meal_name.as_deref(),
        note: note.as_deref(),
        photo: PhotoColumns::of(photo.as_ref()),
    };
    let inserted = insert_log(db, &row, &prepared, &prepared_doses).await;
    let referenced = matches!(inserted, Ok(Some(_)));
    if !referenced && let Some(photo) = &photo {
        photos.remove(&photo.id).await;
    }
    let Some(meal_log) = inserted? else {
        return replay(db, &request_id, &normalized).await?.ok_or_else(|| {
            MealLogError::Conflict(
                "Retry receipt unavailable; list meal_logs before retrying".into(),
            )
        });
    };
    Ok(Logged {
        meal_log,
        replayed: false,
    })
}

struct NewLog<'a> {
    id: &'a str,
    request_id: &'a str,
    normalized: &'a str,
    eaten_at: &'a str,
    day: String,
    meal_name: Option<&'a str>,
    note: Option<&'a str>,
    photo: PhotoColumns,
}

async fn insert_log(
    db: &SqlitePool,
    row: &NewLog<'_>,
    portions: &[PreparedPortion],
    doses: &[StoredDose],
) -> Result<Option<MealLog>> {
    let mut transaction = db.begin().await?;
    let claimed = sqlx::query_scalar!(
        "INSERT INTO meal_log_receipts (request_id, meal_log_id, request) VALUES (?, ?, ?) ON CONFLICT (request_id) DO NOTHING RETURNING request_id",
        row.request_id, row.id, row.normalized)
        .fetch_optional(&mut *transaction)
        .await?;
    if claimed.is_none() {
        transaction.rollback().await?;
        return Ok(None);
    }
    let photo = &row.photo;
    sqlx::query!(
        "INSERT INTO meal_logs (id, eaten_at, day, meal_name, note, photo_id, photo_width, photo_height, photo_thumbnail_width, photo_thumbnail_height) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        row.id, row.eaten_at, row.day, row.meal_name, row.note,
        photo.id, photo.width, photo.height, photo.thumbnail_width, photo.thumbnail_height)
    .execute(&mut *transaction)
    .await?;
    insert_portions(&mut transaction, row.id, portions).await?;
    supplements::insert_log_doses(&mut transaction, row.id, doses).await?;
    let meal_log = load_on(
        &mut transaction,
        &Selection::One(row.id),
        Values::Only(&KEY_NUTRIENTS),
    )
    .await?
    .into_iter()
    .next()
    .ok_or_else(not_found)?
    .into_meal_log()?;
    let result =
        serde_json::to_string(&meal_log).map_err(|error| sqlx::Error::Encode(Box::new(error)))?;
    sqlx::query!(
        "UPDATE meal_log_receipts SET result = ? WHERE request_id = ?",
        result,
        row.request_id
    )
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(Some(meal_log))
}
