use std::collections::HashMap;
use std::iter::Sum;
use std::ops::Add;

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use super::food_database::{Food, FoodDatabase, FoodError, FoodSearch, FoodSummary};
use super::meal_logs::{StoredPortion, round};
use super::parameters;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct NovaClass(u8);

impl<'de> Deserialize<'de> for NovaClass {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::new(i64::deserialize(deserializer)?)
            .ok_or_else(|| serde::de::Error::custom("NOVA Class must be 1-4"))
    }
}

impl NovaClass {
    #[must_use]
    pub fn new(value: i64) -> Option<Self> {
        u8::try_from(value)
            .ok()
            .filter(|value| (1..=4).contains(value))
            .map(Self)
    }

    #[must_use]
    pub fn get(self) -> u8 {
        self.0
    }

    fn is_unprocessed(self) -> bool {
        self.0 == 1
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClassifyFood {
    pub nova_class: i64,
}

#[derive(Debug, Serialize)]
pub struct ClassifiedFood {
    pub food_id: i64,
    pub name_da: String,
    pub name_en: String,
    pub nova_class: NovaClass,
    pub classified_portions: u64,
}

pub async fn classify_food(
    db: &SqlitePool,
    foods: &FoodDatabase,
    food_id: i64,
    request: ClassifyFood,
) -> Result<ClassifiedFood, FoodError> {
    let nova_class = NovaClass::new(request.nova_class).ok_or_else(|| {
        FoodError::Validation(
            "nova_class must be 1 (unprocessed or minimally processed), 2 (processed culinary \
             ingredient), 3 (processed) or 4 (ultra-processed)"
                .into(),
        )
    })?;
    let food = foods
        .snapshots(&[food_id])
        .await?
        .remove(&food_id)
        .ok_or(FoodError::NotFound(food_id))?;
    let class = i64::from(nova_class.get());
    let mut transaction = db.begin().await?;
    sqlx::query!(
        r#"INSERT INTO food_nova_classes (food_id, dataset_version, nova_class) VALUES (?1, ?2, ?3)
        ON CONFLICT (food_id) DO UPDATE SET dataset_version = excluded.dataset_version,
            nova_class = excluded.nova_class, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')"#,
        food_id,
        food.dataset_version,
        class
    )
    .execute(&mut *transaction)
    .await?;
    let filled = sqlx::query!(
        r#"UPDATE meal_log_portions SET nova_class = ?1
        WHERE food_id = ?2 AND dataset_version = ?3 AND product_barcode IS NULL
            AND nova_class IS NULL"#,
        class,
        food_id,
        food.dataset_version
    )
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(ClassifiedFood {
        food_id,
        name_da: food.name_da,
        name_en: food.name_en,
        nova_class,
        classified_portions: filled.rows_affected(),
    })
}

pub(super) async fn food_classes(
    db: &SqlitePool,
    dataset_version: &str,
    food_ids: &[i64],
) -> Result<HashMap<i64, NovaClass>, sqlx::Error> {
    if food_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let ids =
        serde_json::to_string(food_ids).map_err(|error| sqlx::Error::Encode(Box::new(error)))?;
    let rows = sqlx::query!(
        r#"SELECT food_id AS "food_id!", nova_class FROM food_nova_classes
        WHERE food_id IN (SELECT value FROM json_each(?1)) AND dataset_version = ?2"#,
        ids,
        dataset_version
    )
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .filter_map(|row| Some((row.food_id, NovaClass::new(row.nova_class)?)))
        .collect())
}

pub(super) async fn classify_summaries(
    db: &SqlitePool,
    dataset_version: &str,
    summaries: &mut [FoodSummary],
) -> Result<(), sqlx::Error> {
    let ids: Vec<i64> = summaries.iter().map(|food| food.food_id).collect();
    let classes = food_classes(db, dataset_version, &ids).await?;
    for summary in summaries {
        summary.nova_class = classes.get(&summary.food_id).copied();
    }
    Ok(())
}

pub async fn search_foods(
    db: &SqlitePool,
    foods: &FoodDatabase,
    query: &str,
    limit: Option<usize>,
) -> Result<FoodSearch, FoodError> {
    let mut search = foods.search(query, limit).await?;
    classify_summaries(db, &search.source.version, &mut search.foods).await?;
    Ok(search)
}

pub async fn food(db: &SqlitePool, foods: &FoodDatabase, food_id: i64) -> Result<Food, FoodError> {
    let mut food = foods.food(food_id).await?;
    food.nova_class = food_classes(db, &food.source.version, &[food_id])
        .await?
        .remove(&food_id);
    Ok(food)
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct NovaEnergy {
    nova_1_kcal: f64,
    classified_kcal: f64,
    unclassified_portions: usize,
}

impl NovaEnergy {
    pub(super) fn of(portions: &[&StoredPortion]) -> Self {
        portions
            .iter()
            .map(|portion| match portion.nova_class {
                None => Self {
                    unclassified_portions: 1,
                    ..Self::default()
                },
                Some(class) => {
                    let kcal = portion.amount(parameters::ENERGY_KCAL).unwrap_or(0.0);
                    Self {
                        nova_1_kcal: if class.is_unprocessed() { kcal } else { 0.0 },
                        classified_kcal: kcal,
                        unclassified_portions: 0,
                    }
                }
            })
            .sum()
    }

    #[must_use]
    pub fn share(self) -> Option<f64> {
        (self.classified_kcal > 0.0).then(|| round(self.nova_1_kcal / self.classified_kcal))
    }

    #[must_use]
    pub fn summary(self) -> NovaShare {
        NovaShare {
            nova_1_share: self.share(),
            nova_1_kcal: round(self.nova_1_kcal),
            classified_kcal: round(self.classified_kcal),
            unclassified_portions: self.unclassified_portions,
        }
    }
}

impl Add for NovaEnergy {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self {
            nova_1_kcal: self.nova_1_kcal + other.nova_1_kcal,
            classified_kcal: self.classified_kcal + other.classified_kcal,
            unclassified_portions: self.unclassified_portions + other.unclassified_portions,
        }
    }
}

impl Sum for NovaEnergy {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Self::default(), Add::add)
    }
}

#[derive(Debug, Serialize)]
pub struct NovaShare {
    pub nova_1_share: Option<f64>,
    pub nova_1_kcal: f64,
    pub classified_kcal: f64,
    pub unclassified_portions: usize,
}
