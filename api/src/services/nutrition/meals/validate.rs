use super::{MealError, MealItemRequest, Result, validation};
use crate::services::nutrition::food_database::{FoodDatabase, FoodError};
use crate::services::nutrition::meal_logs::{self, Item, MAX_PORTIONS};
use crate::services::nutrition::products::{self, Barcode};
use crate::services::nutrition::supplements::{self, DoseRequest};
use sqlx::SqlitePool;
use std::collections::HashSet;

pub(in crate::services::nutrition) fn item(
    request: MealItemRequest,
) -> std::result::Result<(Item, f64), String> {
    let item = meal_logs::parse_item(request.food_id, request.barcode)?;
    meal_logs::check_grams(request.grams)?;
    Ok((item, request.grams))
}

pub(super) fn items(requests: Vec<MealItemRequest>) -> Result<Vec<(Item, f64)>> {
    if requests.len() > MAX_PORTIONS {
        return Err(validation(format!(
            "portions must list at most {MAX_PORTIONS} Food or Product items"
        )));
    }
    let items = requests
        .into_iter()
        .map(item)
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(validation)?;
    let mut seen = HashSet::new();
    for (item, _) in &items {
        if !seen.insert(item) {
            return Err(validation(format!(
                "{} is listed twice; list each Food or Product once with its grams added together",
                item.describe()
            )));
        }
    }
    Ok(items)
}

pub(super) fn doses(requests: Vec<DoseRequest>) -> Result<Vec<(String, f64)>> {
    supplements::parse_doses(requests).map_err(validation)
}

pub(super) fn check_items(items: usize, doses: usize) -> Result<()> {
    if items + doses == 0 {
        return Err(validation(
            "a Meal needs at least one Portion or Supplement Dose".into(),
        ));
    }
    Ok(())
}

pub(super) enum CheckedItem {
    Food {
        food_id: i64,
        name_da: String,
        name_en: String,
        dataset_version: String,
        grams: f64,
    },
    Product {
        barcode: Barcode,
        grams: f64,
    },
}

pub(super) async fn check(
    db: &SqlitePool,
    foods: &FoodDatabase,
    items: Vec<(Item, f64)>,
) -> Result<Vec<CheckedItem>> {
    if items.is_empty() {
        return Ok(Vec::new());
    }
    let food_ids: Vec<i64> = items
        .iter()
        .filter_map(|(item, _)| match item {
            Item::Food(food_id) => Some(*food_id),
            Item::Product(_) => None,
        })
        .collect();
    let mut snapshots = foods.snapshots(&food_ids).await?;
    let barcodes: Vec<&Barcode> = items
        .iter()
        .filter_map(|(item, _)| match item {
            Item::Product(barcode) => Some(barcode),
            Item::Food(_) => None,
        })
        .collect();
    let linked = products::stored(db, &barcodes).await?;
    items
        .into_iter()
        .map(|(item, grams)| match item {
            Item::Food(food_id) => {
                let food = snapshots
                    .remove(&food_id)
                    .ok_or(FoodError::NotFound(food_id))?;
                Ok(CheckedItem::Food {
                    food_id,
                    name_da: food.name_da,
                    name_en: food.name_en,
                    dataset_version: food.dataset_version,
                    grams,
                })
            }
            Item::Product(barcode) => {
                if !linked.contains_key(barcode.as_str()) {
                    return Err(MealError::NotFound(format!(
                        "Product {} is not linked; lookup_product it and link_product it to its \
                         closest Food before saving it in a Meal",
                        barcode.as_str()
                    )));
                }
                Ok(CheckedItem::Product { barcode, grams })
            }
        })
        .collect()
}

pub(super) fn normalized(
    name: &str,
    items: &[(Item, f64)],
    doses: &[(String, f64)],
) -> Result<String> {
    let portions: Vec<serde_json::Value> = items
        .iter()
        .map(|(item, grams)| item.normalized(*grams))
        .collect();
    let mut request = serde_json::json!({"name": name, "portions": portions});
    if !doses.is_empty() {
        request["supplement_doses"] = doses
            .iter()
            .map(|(id, doses)| supplements::normalized(id, *doses))
            .collect();
    }
    serde_json::to_string(&request).map_err(|error| sqlx::Error::Encode(Box::new(error)).into())
}

pub(super) async fn check_doses(db: &SqlitePool, doses: &[(String, f64)]) -> Result<()> {
    supplements::resolve(db, doses).await?;
    Ok(())
}
