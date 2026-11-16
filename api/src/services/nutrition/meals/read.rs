use super::{Meal, MealError, MealPortion, Meals, Result};
use crate::services::nutrition::meal_logs::PortionProduct;
use crate::services::nutrition::supplements;
use sqlx::SqlitePool;
use std::collections::HashMap;

/// A stored item as `load` reads it.
pub(super) struct StoredItem {
    pub(super) meal_id: String,
    pub(super) grams: f64,
    pub(super) food_id: Option<i64>,
    pub(super) food_name_da: Option<String>,
    pub(super) food_name_en: Option<String>,
    pub(super) barcode: Option<String>,
    product_name: Option<String>,
    product_brand: Option<String>,
    linked_name_da: Option<String>,
    linked_name_en: Option<String>,
}

impl StoredItem {
    fn into_portion(self) -> MealPortion {
        let product = self.barcode.clone().map(|barcode| PortionProduct {
            barcode,
            name: self.product_name,
            brand: self.product_brand,
        });
        MealPortion {
            food_id: self.food_id,
            barcode: self.barcode,
            grams: self.grams,
            name_da: self
                .food_name_da
                .or(self.linked_name_da)
                .unwrap_or_default(),
            name_en: self
                .food_name_en
                .or(self.linked_name_en)
                .unwrap_or_default(),
            product,
        }
    }
}

/// Every stored item of one Meal, or of every Meal, in order.
pub(super) async fn stored_items_on(
    db: &mut sqlx::SqliteConnection,
    id: Option<&str>,
) -> Result<Vec<StoredItem>> {
    Ok(sqlx::query_as!(
        StoredItem,
        r#"SELECT i.meal_id, i.grams, i.food_id, i.food_name_da, i.food_name_en,
            i.product_barcode AS barcode, p.name AS product_name, p.brand AS product_brand,
            p.food_name_da AS "linked_name_da?", p.food_name_en AS "linked_name_en?"
        FROM meal_portions i LEFT JOIN products p ON p.barcode = i.product_barcode
        WHERE ?1 IS NULL OR i.meal_id = ?1
        ORDER BY i.meal_id, i.position"#,
        id
    )
    .fetch_all(&mut *db)
    .await?)
}

/// One Meal, or every Meal by name.
async fn load(db: &SqlitePool, id: Option<&str>) -> Result<Vec<Meal>> {
    load_on(&mut *db.acquire().await?, id).await
}

pub(super) async fn load_on(
    db: &mut sqlx::SqliteConnection,
    id: Option<&str>,
) -> Result<Vec<Meal>> {
    let meals = sqlx::query!(
        r#"SELECT id AS "id!", name FROM meals WHERE ?1 IS NULL OR id = ?1 ORDER BY name_key"#,
        id
    )
    .fetch_all(&mut *db)
    .await?;
    let mut items: HashMap<String, Vec<MealPortion>> = HashMap::new();
    for item in stored_items_on(&mut *db, id).await? {
        items
            .entry(item.meal_id.clone())
            .or_default()
            .push(item.into_portion());
    }
    let mut doses = supplements::meal_doses(&mut *db, id).await?;
    Ok(meals
        .into_iter()
        .map(|row| Meal {
            portions: items.remove(&row.id).unwrap_or_default(),
            supplement_doses: doses.remove(&row.id).unwrap_or_default(),
            id: row.id,
            name: row.name,
        })
        .collect())
}

pub(super) async fn find(db: &SqlitePool, id: &str) -> Result<Option<Meal>> {
    Ok(load(db, Some(id)).await?.into_iter().next())
}

pub(super) fn not_found() -> MealError {
    MealError::NotFound("Meal not found; list meals for its id".into())
}

/// Every Meal by name, with its items in order.
pub async fn meals(db: &SqlitePool) -> Result<Meals> {
    Ok(Meals {
        meals: load(db, None).await?,
    })
}
