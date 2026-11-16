use super::{MealError, Resolved, Result, UniqueName, stored_items_on, validation};
use crate::services::nutrition::food_database::FoodDatabase;
use crate::services::nutrition::meal_logs::Item;
use crate::services::nutrition::products::Barcode;
use crate::services::nutrition::supplements;
use sqlx::SqlitePool;

/// The Meal named `name` (case-insensitively) with its items and doses to log, each override
/// replacing the grams of the item or the doses of the Supplement it names. A Food the current
/// dataset no longer has, or now names differently, must be replaced with `update_meal` first.
pub(in crate::services::nutrition) async fn resolve(
    db: &SqlitePool,
    foods: &FoodDatabase,
    name: &UniqueName,
    overrides: &[(Item, f64)],
    dose_overrides: &[(String, f64)],
) -> Result<Resolved> {
    let Some(meal) = sqlx::query!(
        r#"SELECT id AS "id!", name FROM meals WHERE name_key = ?"#,
        name.key
    )
    .fetch_optional(db)
    .await?
    else {
        return Err(MealError::NotFound(format!(
            "No Meal is named {}; list meals for its name, or log the meal's portions",
            name.name
        )));
    };
    let stored = stored_items_on(&mut *db.acquire().await?, Some(&meal.id)).await?;
    let mut items: Vec<(Item, f64)> = stored
        .iter()
        .map(|item| {
            let reference = match (&item.food_id, &item.barcode) {
                (Some(food_id), _) => Item::Food(*food_id),
                (None, Some(barcode)) => Item::Product(
                    Barcode::parse(barcode).map_err(|error| validation(error.to_string()))?,
                ),
                (None, None) => {
                    return Err(
                        sqlx::Error::Decode("a Meal item without a reference".into()).into(),
                    );
                }
            };
            Ok((reference, item.grams))
        })
        .collect::<Result<_>>()?;
    for (target, grams) in overrides {
        let Some((_, item_grams)) = items.iter_mut().find(|(item, _)| item == target) else {
            return Err(validation(format!(
                "The Meal {} has no item with {}; overrides name an item by the food_id or \
                 barcode meals lists for it",
                meal.name,
                target.describe()
            )));
        };
        *item_grams = *grams;
    }
    let mut doses: Vec<(String, f64)> =
        supplements::meal_doses(&mut *db.acquire().await?, Some(&meal.id))
            .await?
            .remove(&meal.id)
            .unwrap_or_default()
            .into_iter()
            .map(|dose| (dose.supplement_id, dose.doses))
            .collect();
    for (target, count) in dose_overrides {
        let Some((_, doses)) = doses.iter_mut().find(|(id, _)| id == target) else {
            return Err(validation(format!(
                "The Meal {} has no Supplement Dose with supplement_id {target}; overrides name a \
                 dose by the supplement_id meals lists for it",
                meal.name
            )));
        };
        *doses = *count;
    }

    let food_ids: Vec<i64> = stored.iter().filter_map(|item| item.food_id).collect();
    if food_ids.is_empty() {
        return Ok(Resolved {
            name: meal.name,
            items,
            doses,
        });
    }
    let current = foods.names(&food_ids).await?;
    let stale: Vec<String> = stored
        .iter()
        .filter_map(|item| {
            let food_id = item.food_id?;
            let saved_da = item.food_name_da.as_deref().unwrap_or_default();
            let saved_en = item.food_name_en.as_deref().unwrap_or_default();
            let same_food = current
                .get(&food_id)
                .is_some_and(|(name_da, name_en)| name_da == saved_da || name_en == saved_en);
            (!same_food).then(|| format!("Food {food_id} \"{saved_en}\""))
        })
        .collect();
    if !stale.is_empty() {
        return Err(MealError::NeedsUpdate(format!(
            "The Meal {} names {} that the current Food database no longer has under that \
             food_id; search_foods for each and update_meal its portions, then log it again",
            meal.name,
            stale.join(", ")
        )));
    }
    Ok(Resolved {
        name: meal.name,
        items,
        doses,
    })
}
