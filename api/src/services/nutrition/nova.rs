//! NOVA Classes (`CONTEXT.md`, NOVA Class) and the NOVA 1 share.
//!
//! The agent classifies a Food once, the first time it is eaten, and the class is kept: it is the
//! user's decision, so it lives in `app.db` by `FoodID` and a rebuilt Food database keeps it.
//! Products take Open Food Facts' class and are never classified from their linked Food.
//!
//! A Portion copies its class when it is logged, like its nutrient values (ADR 0012). Because the
//! agent logs a new Food before classifying it, classifying a Food also fills that Food's
//! still-unclassified Food Portions logged from the same dataset version; a `FoodID` from another
//! version may name another Food. A changed class applies to Portions logged afterwards, and
//! replacing a past Portion copies the class it has then.
//!
//! The NOVA 1 share is kcal from Class 1 Portions over kcal from classified Portions; unclassified
//! Portions are left out rather than guessed.

use std::collections::HashMap;
use std::iter::Sum;
use std::ops::Add;

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use super::food_database::{Food, FoodDatabase, FoodError, FoodSearch, FoodSummary};
use super::meal_logs::{StoredPortion, round};
use super::parameters;

/// A NOVA processing class: 1 unprocessed or minimally processed, 2 processed culinary
/// ingredients, 3 processed, 4 ultra-processed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct NovaClass(u8);

impl NovaClass {
    /// The class `value` names, if it is 1-4.
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
    /// 1-4.
    pub nova_class: i64,
}

#[derive(Debug, Serialize)]
pub struct ClassifiedFood {
    pub food_id: i64,
    pub name_da: String,
    pub name_en: String,
    pub nova_class: NovaClass,
    /// Past Portions of the Food that were unclassified and now carry the class.
    pub classified_portions: u64,
}

/// Sets or changes a Food's NOVA Class and fills its unclassified past Food Portions from the
/// current dataset version.
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
        r#"INSERT INTO food_nova_classes (food_id, nova_class) VALUES (?1, ?2)
        ON CONFLICT (food_id) DO UPDATE SET nova_class = excluded.nova_class,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')"#,
        food_id,
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

/// The stored classes of the Foods in `food_ids` that have one.
pub(super) async fn food_classes(
    db: &SqlitePool,
    food_ids: &[i64],
) -> Result<HashMap<i64, NovaClass>, sqlx::Error> {
    if food_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let ids =
        serde_json::to_string(food_ids).map_err(|error| sqlx::Error::Encode(Box::new(error)))?;
    let rows = sqlx::query!(
        r#"SELECT food_id AS "food_id!", nova_class FROM food_nova_classes
        WHERE food_id IN (SELECT value FROM json_each(?))"#,
        ids
    )
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .filter_map(|row| Some((row.food_id, NovaClass::new(row.nova_class)?)))
        .collect())
}

/// Gives each summary its Food's stored class.
pub(super) async fn classify_summaries(
    db: &SqlitePool,
    summaries: &mut [FoodSummary],
) -> Result<(), sqlx::Error> {
    let ids: Vec<i64> = summaries.iter().map(|food| food.food_id).collect();
    let classes = food_classes(db, &ids).await?;
    for summary in summaries {
        summary.nova_class = classes.get(&summary.food_id).copied();
    }
    Ok(())
}

/// Foods matching `query`, best first, each with its NOVA Class when it has one.
pub async fn search_foods(
    db: &SqlitePool,
    foods: &FoodDatabase,
    query: &str,
    limit: Option<usize>,
) -> Result<FoodSearch, FoodError> {
    let mut search = foods.search(query, limit).await?;
    classify_summaries(db, &mut search.foods).await?;
    Ok(search)
}

/// One Food with every parameter and its NOVA Class when it has one.
pub async fn food(db: &SqlitePool, foods: &FoodDatabase, food_id: i64) -> Result<Food, FoodError> {
    let mut food = foods.food(food_id).await?;
    food.nova_class = food_classes(db, &[food_id]).await?.remove(&food_id);
    Ok(food)
}

/// The energy behind the NOVA 1 share, summed over Portions. Sums add, so a day, week or month
/// is the sum of its Portions or days and its share follows from the sum.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct NovaEnergy {
    nova_1_kcal: f64,
    classified_kcal: f64,
    unclassified_portions: usize,
}

impl NovaEnergy {
    /// The energy of `portions`; a classified Portion without an energy value adds nothing.
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

    /// kcal from Class 1 Portions over kcal from classified Portions, 0-1; `None` when no
    /// classified Portion has energy.
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

/// The NOVA 1 share of a span with the energy it is computed from.
#[derive(Debug, Serialize)]
pub struct NovaShare {
    /// kcal from NOVA Class 1 Portions ÷ kcal from classified Portions, 0-1; null when nothing
    /// classified has energy.
    pub nova_1_share: Option<f64>,
    pub nova_1_kcal: f64,
    pub classified_kcal: f64,
    /// Portions left out because neither their Food nor their Product has a class.
    pub unclassified_portions: usize,
}
