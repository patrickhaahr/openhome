use super::{
    Item, MealLogError, PortionProduct, Result, ValidPortion, ValueSource, vocabulary_unit,
};
use crate::services::nutrition::food_database::{FoodDatabase, FoodError};
use crate::services::nutrition::nova::{self, NovaClass};
use crate::services::nutrition::products::{self, Barcode};
use crate::services::nutrition::supplements::{self, StoredDose};
use serde::Serialize;
use sqlx::SqlitePool;
use std::collections::HashMap;

#[derive(Serialize)]
struct PreparedValue {
    key: String,
    value: f64,
    unit: String,
    source: ValueSource,
}

pub(super) struct PreparedPortion {
    grams: f64,
    food_id: i64,
    name_da: String,
    name_en: String,
    dataset_version: String,
    product: Option<PortionProduct>,
    nova_class: Option<NovaClass>,
    values: Vec<PreparedValue>,
}

fn product_needs_relink(product: &products::StoredProduct) -> MealLogError {
    MealLogError::ProductNeedsRelink(format!(
        "Product {} is linked to Food {} \"{}\", which the current Food database no longer has under that food_id; search_foods for its closest Food, link_product it again, then log it",
        product.barcode,
        product.food_id,
        product.food_name_en()
    ))
}

pub(super) async fn prepare(
    db: &SqlitePool,
    foods: &FoodDatabase,
    portions: &[ValidPortion],
) -> Result<Vec<PreparedPortion>> {
    if portions.is_empty() {
        return Ok(Vec::new());
    }
    let barcodes: Vec<&Barcode> = portions
        .iter()
        .filter_map(|portion| match &portion.item {
            Item::Product(barcode) => Some(barcode),
            Item::Food(_) => None,
        })
        .collect();
    let stored = products::stored(db, &barcodes).await?;
    let classified_foods: Vec<i64> = portions
        .iter()
        .filter_map(|portion| match &portion.item {
            Item::Food(food_id) => Some(*food_id),
            Item::Product(_) => None,
        })
        .collect();
    let mut items = Vec::with_capacity(portions.len());
    for portion in portions {
        items.push(match &portion.item {
            Item::Food(food_id) => (*food_id, None),
            Item::Product(barcode) => {
                let product = stored.get(barcode.as_str()).ok_or_else(|| {
                    MealLogError::NotFound(format!(
                        "Product {} is not linked; lookup_product it and link_product it to its \
                         closest Food first",
                        barcode.as_str()
                    ))
                })?;
                (product.food_id, Some(product))
            }
        });
    }
    let food_ids: Vec<i64> = items.iter().map(|(food_id, _)| *food_id).collect();
    let snapshots = foods.snapshots(&food_ids).await.map_err(|error| {
        if let FoodError::NotFound(food_id) = &error
            && let Some(product) = stored.values().find(|product| product.food_id == *food_id)
        {
            return product_needs_relink(product);
        }
        MealLogError::Food(error)
    })?;
    let classes = match snapshots.values().next() {
        Some(food) => nova::food_classes(db, &food.dataset_version, &classified_foods).await?,
        None => HashMap::new(),
    };
    portions
        .iter()
        .zip(items)
        .map(|(portion, (food_id, product))| {
            let food = snapshots.get(&food_id);
            if let Some(product) = product
                && !food.is_some_and(|food| product.links(food))
            {
                return Err(product_needs_relink(product));
            }
            let food = food.ok_or(FoodError::NotFound(food_id))?;
            let mut values: Vec<PreparedValue> = product
                .iter()
                .flat_map(|product| &product.label)
                .map(|label| PreparedValue {
                    key: label.key.clone(),
                    value: label.value,
                    unit: label.unit.clone(),
                    source: ValueSource::Label,
                })
                .collect();
            for value in &food.per_100g {
                if !values.iter().any(|known| known.key == value.key) {
                    values.push(PreparedValue {
                        key: value.key.clone(),
                        value: value.value,
                        unit: value.unit.clone(),
                        source: ValueSource::Food,
                    });
                }
            }
            for (key, value) in &portion.estimates {
                if let Some(known) = values.iter().find(|known| &known.key == key) {
                    let holder = match known.source {
                        ValueSource::Label => "the Product's label",
                        _ => "the Food",
                    };
                    return Err(MealLogError::Validation(format!(
                        "estimates.{key}: {holder} already has a value; estimate only values no \
                         source has"
                    )));
                }
                values.push(PreparedValue {
                    key: key.clone(),
                    value: *value,
                    unit: vocabulary_unit(key).unwrap_or_default().to_owned(),
                    source: ValueSource::Estimate,
                });
            }
            Ok(PreparedPortion {
                grams: portion.grams,
                food_id: food.food_id,
                name_da: food.name_da.clone(),
                name_en: food.name_en.clone(),
                dataset_version: food.dataset_version.clone(),
                nova_class: match product {
                    Some(product) => product
                        .nova_class
                        .and_then(|class| NovaClass::new(class.into())),
                    None => classes.get(&food_id).copied(),
                },
                product: product.map(|product| PortionProduct {
                    barcode: product.barcode.clone(),
                    name: product.name.clone(),
                    brand: product.brand.clone(),
                }),
                values,
            })
        })
        .collect()
}

pub(super) async fn insert_portions(
    transaction: &mut sqlx::SqliteConnection,
    meal_log_id: &str,
    portions: &[PreparedPortion],
) -> Result<()> {
    for (position, portion) in portions.iter().enumerate() {
        let position = position as i64;
        let product = portion.product.as_ref();
        let barcode = product.map(|product| product.barcode.as_str());
        let name = product.and_then(|product| product.name.as_deref());
        let brand = product.and_then(|product| product.brand.as_deref());
        let nova_class = portion.nova_class.map(|class| i64::from(class.get()));
        let portion_id = sqlx::query_scalar!(
            "INSERT INTO meal_log_portions (meal_log_id, position, grams, food_id, name_da, name_en, dataset_version, product_barcode, product_name, product_brand, nova_class) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
            meal_log_id, position, portion.grams, portion.food_id, portion.name_da, portion.name_en, portion.dataset_version, barcode, name, brand, nova_class)
            .fetch_one(&mut *transaction)
            .await?;
        let values = serde_json::to_string(&portion.values)
            .map_err(|error| sqlx::Error::Encode(Box::new(error)))?;
        sqlx::query!(
            "INSERT INTO meal_log_portion_values (portion_id, key, value, unit, source) SELECT ?, json_extract(value, '$.key'), json_extract(value, '$.value'), json_extract(value, '$.unit'), json_extract(value, '$.source') FROM json_each(?)",
            portion_id, values)
            .execute(&mut *transaction)
            .await?;
    }
    Ok(())
}

pub(super) async fn prepare_doses(
    db: &SqlitePool,
    doses: &[(String, f64)],
) -> Result<Vec<StoredDose>> {
    Ok(supplements::resolve(db, doses)
        .await?
        .into_iter()
        .map(|(supplement, doses)| StoredDose::snapshot(supplement, doses))
        .collect())
}
