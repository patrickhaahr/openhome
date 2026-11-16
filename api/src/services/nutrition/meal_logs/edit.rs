use super::{
    EditMealLog, MealLog, PhotoColumns, PreparedPortion, Result, check_items, doses, eaten_at,
    find, insert_portions, not_found, note, portions, prepare, prepare_doses, required,
    store_photo, utc_text,
};
use crate::services::nutrition::food_database::FoodDatabase;
use crate::services::nutrition::meal_photos::{MealPhotos, Upload};
use crate::services::nutrition::supplements::{self, StoredDose};
use crate::services::patch::Patch;
use crate::services::training::calendar::CalendarDay;
use chrono::{DateTime, Utc};
use sqlx::SqlitePool;

/// What an edit does to the photo.
enum PhotoChange {
    Keep,
    Remove,
    Replace(Upload),
}

/// The validated changes of one edit, applied together.
struct Changes {
    eaten_at: Option<DateTime<Utc>>,
    /// `Some` when the note is supplied; its value may clear it.
    note: Option<Option<String>>,
    portions: Option<Vec<PreparedPortion>>,
    doses: Option<Vec<StoredDose>>,
    photo: PhotoChange,
}

/// Applies the supplied fields together. Replacing Portions or doses snapshots their Foods or
/// Supplements afresh; replacing the photo encodes it before anything is written and removes the
/// replaced files.
pub async fn update_meal_log(
    db: &SqlitePool,
    foods: &FoodDatabase,
    photos: &MealPhotos,
    id: &str,
    edit: EditMealLog,
) -> Result<MealLog> {
    let at = required(edit.eaten_at, "eaten_at")?
        .map(|value| eaten_at(&value, Utc::now()))
        .transpose()?;
    let note = match edit.note {
        Patch::Missing => None,
        Patch::Value(value) => Some(note(value)?),
    };
    let replacement = required(edit.portions, "portions")?
        .map(portions)
        .transpose()?;
    let dose_replacement = required(edit.supplement_doses, "supplement_doses")?
        .map(doses)
        .transpose()?;
    let photo = match edit.photo {
        Patch::Missing => PhotoChange::Keep,
        Patch::Value(None) => PhotoChange::Remove,
        Patch::Value(Some(text)) => PhotoChange::Replace(Upload::from_base64(&text)?),
    };
    let current = find(db, id).await?.ok_or_else(not_found)?;
    check_items(
        replacement
            .as_ref()
            .map_or(current.portions.len(), Vec::len),
        dose_replacement
            .as_ref()
            .map_or(current.supplement_doses.len(), Vec::len),
    )?;
    let portions = match replacement {
        Some(requests) => Some(prepare(db, foods, &requests).await?),
        None => None,
    };
    let doses = match dose_replacement {
        Some(doses) => Some(prepare_doses(db, &doses).await?),
        None => None,
    };
    apply(
        db,
        photos,
        id,
        Changes {
            eaten_at: at,
            note,
            portions,
            doses,
            photo,
        },
    )
    .await
}

/// Attaches `upload` as the Meal Log's photo, replacing and removing any earlier photo.
pub async fn replace_photo(
    db: &SqlitePool,
    photos: &MealPhotos,
    id: &str,
    upload: Upload,
) -> Result<MealLog> {
    if find(db, id).await?.is_none() {
        return Err(not_found());
    }
    let changes = Changes {
        eaten_at: None,
        note: None,
        portions: None,
        doses: None,
        photo: PhotoChange::Replace(upload),
    };
    apply(db, photos, id, changes).await
}

/// Stores a replacement photo, writes `changes`, then removes the photo they replaced. A failed
/// write removes the new photo instead, so a rejected edit leaves no file behind.
async fn apply(
    db: &SqlitePool,
    photos: &MealPhotos,
    id: &str,
    changes: Changes,
) -> Result<MealLog> {
    let (photo_supplied, photo) = match changes.photo {
        PhotoChange::Keep => (false, None),
        PhotoChange::Remove => (true, None),
        PhotoChange::Replace(upload) => (true, Some(store_photo(photos, upload).await?)),
    };
    let written = write_changes(
        db,
        id,
        Written {
            at: changes.eaten_at,
            note: changes.note.as_ref(),
            portions: changes.portions.as_deref(),
            doses: changes.doses.as_deref(),
            photo: photo_supplied.then(|| PhotoColumns::of(photo.as_ref())),
        },
    )
    .await;
    match written {
        Ok(replaced) => {
            if let Some(replaced) = replaced {
                photos.remove(&replaced).await;
            }
        }
        Err(error) => {
            if let Some(photo) = &photo {
                photos.remove(&photo.id).await;
            }
            return Err(error);
        }
    }
    find(db, id).await?.ok_or_else(not_found)
}

/// The prepared changes of one edit; `None` keeps a field.
struct Written<'a> {
    at: Option<DateTime<Utc>>,
    note: Option<&'a Option<String>>,
    portions: Option<&'a [PreparedPortion]>,
    doses: Option<&'a [StoredDose]>,
    photo: Option<PhotoColumns>,
}

/// Writes one edit in a transaction. Returns the ID of the photo it replaced or removed.
async fn write_changes(db: &SqlitePool, id: &str, changes: Written<'_>) -> Result<Option<String>> {
    let Written {
        at,
        note,
        portions,
        doses,
        photo,
    } = changes;
    let stored_at = at.map(utc_text);
    let day = at.map(|at| CalendarDay::of(at).to_string());
    let note_supplied = note.is_some();
    let note = note.cloned().flatten();
    let photo_supplied = photo.is_some();
    let photo = photo.unwrap_or_default();
    let mut transaction = db.begin().await?;
    let Some(previous) = sqlx::query_scalar!("SELECT photo_id FROM meal_logs WHERE id = ?", id)
        .fetch_optional(&mut *transaction)
        .await?
    else {
        return Err(not_found());
    };
    sqlx::query!(
        r#"UPDATE meal_logs SET eaten_at = COALESCE(?1, eaten_at), day = COALESCE(?2, day),
        note = CASE WHEN ?3 THEN ?4 ELSE note END,
        photo_id = CASE WHEN ?5 THEN ?6 ELSE photo_id END,
        photo_width = CASE WHEN ?5 THEN ?7 ELSE photo_width END,
        photo_height = CASE WHEN ?5 THEN ?8 ELSE photo_height END,
        photo_thumbnail_width = CASE WHEN ?5 THEN ?9 ELSE photo_thumbnail_width END,
        photo_thumbnail_height = CASE WHEN ?5 THEN ?10 ELSE photo_thumbnail_height END,
        updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE id = ?11"#,
        stored_at,
        day,
        note_supplied,
        note,
        photo_supplied,
        photo.id,
        photo.width,
        photo.height,
        photo.thumbnail_width,
        photo.thumbnail_height,
        id
    )
    .execute(&mut *transaction)
    .await?;
    if let Some(portions) = portions {
        sqlx::query!("DELETE FROM meal_log_portions WHERE meal_log_id = ?", id)
            .execute(&mut *transaction)
            .await?;
        insert_portions(&mut transaction, id, portions).await?;
    }
    if let Some(doses) = doses {
        sqlx::query!(
            "DELETE FROM meal_log_supplement_doses WHERE meal_log_id = ?",
            id
        )
        .execute(&mut *transaction)
        .await?;
        supplements::insert_log_doses(&mut transaction, id, doses).await?;
    }
    transaction.commit().await?;
    Ok(previous.filter(|_| photo_supplied))
}

/// Deletes a Meal Log, its Portions and its photo files. Its receipt stays, so an old retry
/// cannot recreate it.
pub async fn delete_meal_log(db: &SqlitePool, photos: &MealPhotos, id: &str) -> Result<()> {
    let Some(photo_id) =
        sqlx::query_scalar!("DELETE FROM meal_logs WHERE id = ? RETURNING photo_id", id)
            .fetch_optional(db)
            .await?
    else {
        return Err(not_found());
    };
    if let Some(photo_id) = photo_id {
        photos.remove(&photo_id).await;
    }
    Ok(())
}
