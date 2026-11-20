use super::{
    CheckedItem, EditMeal, Meal, MealError, Result, SaveMeal, Saved, UniqueName, check,
    check_doses, check_items, doses, find, items, load_on, normalized, not_found, request_id,
    validation,
};
use crate::services::nutrition::food_database::FoodDatabase;
use crate::services::nutrition::supplements;
use crate::services::patch::Patch;
use sqlx::SqlitePool;
use uuid::Uuid;

async fn replay(db: &SqlitePool, request_id: &str, request: &str) -> Result<Option<Saved>> {
    let Some(receipt) = sqlx::query!(
        "SELECT meal_id, request, result FROM meal_receipts WHERE request_id = ?",
        request_id
    )
    .fetch_optional(db)
    .await?
    else {
        return Ok(None);
    };
    if receipt.request != request {
        return Err(MealError::Conflict(
            "request_id was already used for a different Meal; use a new ID for a new Meal".into(),
        ));
    }
    find(db, &receipt.meal_id).await?.ok_or_else(|| {
        MealError::Conflict(
            "The retried Meal was deleted; use a new request_id to save it again".into(),
        )
    })?;
    let result = receipt.result.ok_or_else(|| {
        MealError::Conflict(
            "This legacy receipt has no original result; read the current meal before retrying"
                .into(),
        )
    })?;
    let meal =
        serde_json::from_str(&result).map_err(|error| sqlx::Error::Decode(Box::new(error)))?;
    Ok(Some(Saved {
        meal,
        replayed: true,
    }))
}

fn name_taken(name: &str) -> MealError {
    MealError::Conflict(format!(
        "A Meal named {name} already exists; choose another name, or update_meal that Meal"
    ))
}

fn unique_name(error: sqlx::Error, name: &str) -> MealError {
    match &error {
        sqlx::Error::Database(database) if database.is_unique_violation() => name_taken(name),
        _ => error.into(),
    }
}

async fn insert_items(
    transaction: &mut sqlx::SqliteConnection,
    meal_id: &str,
    items: &[CheckedItem],
) -> Result<()> {
    for (position, item) in items.iter().enumerate() {
        let position = position as i64;
        match item {
            CheckedItem::Food {
                food_id,
                name_da,
                name_en,
                dataset_version,
                grams,
            } => {
                sqlx::query!(
                    "INSERT INTO meal_portions (meal_id, position, grams, food_id, food_name_da, food_name_en, dataset_version) VALUES (?, ?, ?, ?, ?, ?, ?)",
                    meal_id, position, grams, food_id, name_da, name_en, dataset_version)
                    .execute(&mut *transaction)
                    .await?;
            }
            CheckedItem::Product { barcode, grams } => {
                let barcode = barcode.as_str();
                sqlx::query!(
                    "INSERT INTO meal_portions (meal_id, position, grams, product_barcode) VALUES (?, ?, ?, ?)",
                    meal_id, position, grams, barcode)
                    .execute(&mut *transaction)
                    .await?;
            }
        }
    }
    Ok(())
}

pub async fn save_meal(db: &SqlitePool, foods: &FoodDatabase, request: SaveMeal) -> Result<Saved> {
    let request_id = request_id(&request.request_id)?;
    let name = UniqueName::parse(&request.name, "name", "Meal").map_err(validation)?;
    let items = items(request.portions)?;
    let doses = doses(request.supplement_doses)?;
    check_items(items.len(), doses.len())?;
    let normalized = normalized(&name.name, &items, &doses)?;
    if let Some(saved) = replay(db, &request_id, &normalized).await? {
        return Ok(saved);
    }
    let checked = check(db, foods, items).await?;
    check_doses(db, &doses).await?;

    let id = Uuid::new_v4().to_string();
    let mut transaction = db.begin().await?;
    let claimed = sqlx::query_scalar!(
        "INSERT INTO meal_receipts (request_id, meal_id, request) VALUES (?, ?, ?) ON CONFLICT (request_id) DO NOTHING RETURNING request_id",
        request_id, id, normalized)
        .fetch_optional(&mut *transaction)
        .await?;
    if claimed.is_none() {
        transaction.rollback().await?;
        return replay(db, &request_id, &normalized).await?.ok_or_else(|| {
            MealError::Conflict("Retry receipt unavailable; list meals before retrying".into())
        });
    }
    sqlx::query!(
        "INSERT INTO meals (id, name, name_key) VALUES (?, ?, ?)",
        id,
        name.name,
        name.key
    )
    .execute(&mut *transaction)
    .await
    .map_err(|error| unique_name(error, &name.name))?;
    insert_items(&mut transaction, &id, &checked).await?;
    supplements::insert_meal_doses(&mut transaction, &id, &doses).await?;
    let meal = load_on(&mut transaction, Some(&id))
        .await?
        .into_iter()
        .next()
        .ok_or_else(not_found)?;
    let result =
        serde_json::to_string(&meal).map_err(|error| sqlx::Error::Encode(Box::new(error)))?;
    sqlx::query!(
        "UPDATE meal_receipts SET result = ? WHERE request_id = ?",
        result,
        request_id
    )
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(Saved {
        meal,
        replayed: false,
    })
}

fn required<T>(patch: Patch<T>, field: &str) -> Result<Option<T>> {
    match patch {
        Patch::Missing => Ok(None),
        Patch::Value(Some(value)) => Ok(Some(value)),
        Patch::Value(None) => Err(validation(format!("{field} cannot be null"))),
    }
}

pub async fn update_meal(
    db: &SqlitePool,
    foods: &FoodDatabase,
    id: &str,
    edit: EditMeal,
) -> Result<Meal> {
    let name = required(edit.name, "name")?
        .map(|name| UniqueName::parse(&name, "name", "Meal").map_err(validation))
        .transpose()?;
    let items = required(edit.portions, "portions")?
        .map(items)
        .transpose()?;
    let doses = required(edit.supplement_doses, "supplement_doses")?
        .map(doses)
        .transpose()?;
    let current = find(db, id).await?.ok_or_else(not_found)?;
    check_items(
        items.as_ref().map_or(current.portions.len(), Vec::len),
        doses
            .as_ref()
            .map_or(current.supplement_doses.len(), Vec::len),
    )?;
    let checked = match items {
        Some(items) => Some(check(db, foods, items).await?),
        None => None,
    };
    if let Some(doses) = &doses {
        check_doses(db, doses).await?;
    }

    let mut transaction = db.begin().await?;
    let (display, key) = name
        .as_ref()
        .map(|name| (name.name.as_str(), name.key.as_str()))
        .unzip();
    let updated = sqlx::query!(
        r#"UPDATE meals SET name = COALESCE(?1, name), name_key = COALESCE(?2, name_key),
        updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?3"#,
        display,
        key,
        id
    )
    .execute(&mut *transaction)
    .await
    .map_err(|error| unique_name(error, display.unwrap_or_default()))?;
    if updated.rows_affected() == 0 {
        return Err(not_found());
    }
    if let Some(checked) = &checked {
        sqlx::query!("DELETE FROM meal_portions WHERE meal_id = ?", id)
            .execute(&mut *transaction)
            .await?;
        insert_items(&mut transaction, id, checked).await?;
    }
    if let Some(doses) = &doses {
        sqlx::query!("DELETE FROM meal_supplement_doses WHERE meal_id = ?", id)
            .execute(&mut *transaction)
            .await?;
        supplements::insert_meal_doses(&mut transaction, id, doses).await?;
    }
    transaction.commit().await?;
    find(db, id).await?.ok_or_else(not_found)
}

pub async fn delete_meal(db: &SqlitePool, id: &str) -> Result<()> {
    let deleted = sqlx::query!("DELETE FROM meals WHERE id = ?", id)
        .execute(db)
        .await?;
    if deleted.rows_affected() == 0 {
        return Err(not_found());
    }
    Ok(())
}
