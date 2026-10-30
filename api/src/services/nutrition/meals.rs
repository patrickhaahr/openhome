//! Meals (`CONTEXT.md`, Meal): named, reusable lists of Portions the user logs again by name,
//! without analysing a photo.
//!
//! A Meal holds references, never values: each Portion is grams of a Food (by `FoodID`) or of a
//! linked Product (by barcode), in the order the user gave. Logging a Meal snapshots its items
//! afresh (ADR 0012), so a re-linked Product or a corrected Food reaches the next log, and
//! editing or deleting a Meal never changes a Meal Log made from it.
//!
//! Names are unique case-insensitively, including non-ASCII letters. Saving follows the
//! `request_id` receipt pattern: a retry replays the current Meal, the same `request_id` with
//! changed arguments is a conflict, and a deleted Meal keeps its receipt so an old retry cannot
//! recreate it.
//!
//! Every item is checked when it is saved: a Food must be in the current Food database and a
//! barcode must be a linked Product. A Food keeps the names and dataset version it was saved
//! with; when a later dataset no longer has its `FoodID`, or gives that `FoodID` to another Food,
//! the Meal must be updated before it can be logged again ([`MealError::NeedsUpdate`]).

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use uuid::Uuid;

use super::food_database::{FoodDatabase, FoodError};
use super::meal_logs::{self, Item, MAX_PORTIONS, PortionProduct};
use super::products::{self, Barcode};
use crate::services::patch::Patch;

pub const MAX_NAME_CHARS: usize = 100;
const MAX_REQUEST_ID_CHARS: usize = 128;

#[derive(Debug, thiserror::Error)]
pub enum MealError {
    #[error("{0}")]
    Validation(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Conflict(String),
    /// The Meal names a Food the current Food database no longer has.
    #[error("{0}")]
    NeedsUpdate(String),
    #[error(transparent)]
    Food(#[from] FoodError),
    #[error("Meal database operation failed")]
    Database(#[from] sqlx::Error),
}

type Result<T> = std::result::Result<T, MealError>;

/// One item of a Meal: edible grams of a Food (`food_id`) or of a linked Product (`barcode`).
/// The same shape addresses an item, by its `food_id` or `barcode`, in a log's gram overrides.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MealItemRequest {
    #[serde(default)]
    pub food_id: Option<i64>,
    #[serde(default)]
    pub barcode: Option<String>,
    pub grams: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SaveMeal {
    pub name: String,
    pub portions: Vec<MealItemRequest>,
    pub request_id: String,
}

/// Omitted fields keep their value. `portions` replaces every item.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EditMeal {
    #[serde(default)]
    pub name: Patch<String>,
    #[serde(default)]
    pub portions: Patch<Vec<MealItemRequest>>,
}

/// One item of a Meal as it is shown: its reference and grams, with names to recognise it by.
#[derive(Debug, Serialize)]
pub struct MealPortion {
    /// The Food; null for a Product.
    pub food_id: Option<i64>,
    /// The Product's barcode; null for a Food.
    pub barcode: Option<String>,
    pub grams: f64,
    /// The Food as it was named when saved, or the Food the Product is linked to now.
    pub name_da: String,
    pub name_en: String,
    /// The Product as its label names it; null for a Food.
    pub product: Option<PortionProduct>,
}

#[derive(Debug, Serialize)]
pub struct Meal {
    pub id: String,
    pub name: String,
    /// In the order they were given.
    pub portions: Vec<MealPortion>,
}

#[derive(Debug, Serialize)]
pub struct Saved {
    pub meal: Meal,
    pub replayed: bool,
}

#[derive(Debug, Serialize)]
pub struct Meals {
    /// By name.
    pub meals: Vec<Meal>,
}

/// A Meal to log: its name as saved and its items with the grams to log.
pub(super) struct Resolved {
    pub name: String,
    pub items: Vec<(Item, f64)>,
}

/// A Meal name trimmed with inner whitespace collapsed, and the lowercased key names are unique
/// by.
pub(super) struct MealName {
    pub name: String,
    pub key: String,
}

impl MealName {
    pub(super) fn parse(text: &str, field: &str) -> std::result::Result<Self, String> {
        let name = text.split_whitespace().collect::<Vec<_>>().join(" ");
        if name.is_empty()
            || name.chars().count() > MAX_NAME_CHARS
            || name.chars().any(char::is_control)
        {
            return Err(format!(
                "{field} must be a Meal name of 1-{MAX_NAME_CHARS} characters without control \
                 characters"
            ));
        }
        let key = name.to_lowercase();
        Ok(Self { name, key })
    }
}

fn validation(message: String) -> MealError {
    MealError::Validation(message)
}

fn request_id(value: &str) -> Result<String> {
    let value = value.trim();
    if value.is_empty()
        || value.chars().count() > MAX_REQUEST_ID_CHARS
        || value.chars().any(char::is_control)
    {
        return Err(validation(format!(
            "request_id must contain 1-{MAX_REQUEST_ID_CHARS} characters without control characters"
        )));
    }
    Ok(value.to_owned())
}

/// Parses an item reference with its grams.
pub(super) fn item(request: MealItemRequest) -> std::result::Result<(Item, f64), String> {
    let item = meal_logs::parse_item(request.food_id, request.barcode)?;
    meal_logs::check_grams(request.grams)?;
    Ok((item, request.grams))
}

/// Parses 1-50 items, each Food or Product listed once.
fn items(requests: Vec<MealItemRequest>) -> Result<Vec<(Item, f64)>> {
    if requests.is_empty() || requests.len() > MAX_PORTIONS {
        return Err(validation(format!(
            "portions must list 1-{MAX_PORTIONS} Food or Product items"
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

/// A Meal item ready to write: a Food with the names it is saved with, or a linked Product.
enum CheckedItem {
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

/// Checks every item against the current Food database and the linked Products.
async fn check(
    db: &SqlitePool,
    foods: &FoodDatabase,
    items: Vec<(Item, f64)>,
) -> Result<Vec<CheckedItem>> {
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

/// The arguments a receipt compares on retry.
fn normalized(name: &str, items: &[(Item, f64)]) -> Result<String> {
    let portions: Vec<serde_json::Value> = items
        .iter()
        .map(|(item, grams)| item.normalized(*grams))
        .collect();
    serde_json::to_string(&serde_json::json!({"name": name, "portions": portions}))
        .map_err(|error| sqlx::Error::Encode(Box::new(error)).into())
}

/// A stored item as `load` reads it.
struct StoredItem {
    meal_id: String,
    grams: f64,
    food_id: Option<i64>,
    food_name_da: Option<String>,
    food_name_en: Option<String>,
    barcode: Option<String>,
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
async fn stored_items(db: &SqlitePool, id: Option<&str>) -> Result<Vec<StoredItem>> {
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
    .fetch_all(db)
    .await?)
}

/// One Meal, or every Meal by name.
async fn load(db: &SqlitePool, id: Option<&str>) -> Result<Vec<Meal>> {
    let meals = sqlx::query!(
        r#"SELECT id AS "id!", name FROM meals WHERE ?1 IS NULL OR id = ?1 ORDER BY name_key"#,
        id
    )
    .fetch_all(db)
    .await?;
    let mut items: HashMap<String, Vec<MealPortion>> = HashMap::new();
    for item in stored_items(db, id).await? {
        items
            .entry(item.meal_id.clone())
            .or_default()
            .push(item.into_portion());
    }
    Ok(meals
        .into_iter()
        .map(|row| Meal {
            portions: items.remove(&row.id).unwrap_or_default(),
            id: row.id,
            name: row.name,
        })
        .collect())
}

async fn find(db: &SqlitePool, id: &str) -> Result<Option<Meal>> {
    Ok(load(db, Some(id)).await?.into_iter().next())
}

fn not_found() -> MealError {
    MealError::NotFound("Meal not found; list meals for its id".into())
}

/// Every Meal by name, with its items in order.
pub async fn meals(db: &SqlitePool) -> Result<Meals> {
    Ok(Meals {
        meals: load(db, None).await?,
    })
}

async fn replay(db: &SqlitePool, request_id: &str, request: &str) -> Result<Option<Saved>> {
    let Some(receipt) = sqlx::query!(
        "SELECT meal_id, request FROM meal_receipts WHERE request_id = ?",
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
    let meal = find(db, &receipt.meal_id).await?.ok_or_else(|| {
        MealError::Conflict(
            "The retried Meal was deleted; use a new request_id to save it again".into(),
        )
    })?;
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

/// Maps the unique name index to a conflict.
fn unique_name(error: sqlx::Error, name: &str) -> MealError {
    match &error {
        sqlx::Error::Database(database) if database.is_unique_violation() => name_taken(name),
        _ => error.into(),
    }
}

/// Writes a Meal's items, replacing none: the caller removes the previous items first.
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

/// Saves a named Meal of Food and Product items, with an immutable receipt for its
/// `request_id`. Rejected requests consume no `request_id` and write nothing.
pub async fn save_meal(db: &SqlitePool, foods: &FoodDatabase, request: SaveMeal) -> Result<Saved> {
    let request_id = request_id(&request.request_id)?;
    let name = MealName::parse(&request.name, "name").map_err(validation)?;
    let items = items(request.portions)?;
    let normalized = normalized(&name.name, &items)?;
    if let Some(saved) = replay(db, &request_id, &normalized).await? {
        return Ok(saved);
    }
    let checked = check(db, foods, items).await?;

    let id = Uuid::new_v4().to_string();
    let mut transaction = db.begin().await?;
    // Claim first so simultaneous retries serialize on the receipt.
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
    transaction.commit().await?;
    Ok(Saved {
        meal: find(db, &id).await?.ok_or_else(not_found)?,
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

/// Renames a Meal or replaces its items, together. Meal Logs made from it keep their Portions
/// and the name they were logged with.
pub async fn update_meal(
    db: &SqlitePool,
    foods: &FoodDatabase,
    id: &str,
    edit: EditMeal,
) -> Result<Meal> {
    let name = required(edit.name, "name")?
        .map(|name| MealName::parse(&name, "name").map_err(validation))
        .transpose()?;
    let items = required(edit.portions, "portions")?
        .map(items)
        .transpose()?;
    if find(db, id).await?.is_none() {
        return Err(not_found());
    }
    let checked = match items {
        Some(items) => Some(check(db, foods, items).await?),
        None => None,
    };

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
    transaction.commit().await?;
    find(db, id).await?.ok_or_else(not_found)
}

/// Deletes a Meal. Meal Logs made from it are unchanged, and its receipt stays, so an old retry
/// cannot recreate it.
pub async fn delete_meal(db: &SqlitePool, id: &str) -> Result<()> {
    let deleted = sqlx::query!("DELETE FROM meals WHERE id = ?", id)
        .execute(db)
        .await?;
    if deleted.rows_affected() == 0 {
        return Err(not_found());
    }
    Ok(())
}

/// The Meal named `name` (case-insensitively) with its items to log, each override replacing
/// the grams of the item it names. A Food the current dataset no longer has, or now names
/// differently, must be replaced with `update_meal` first.
pub(super) async fn resolve(
    db: &SqlitePool,
    foods: &FoodDatabase,
    name: &MealName,
    overrides: &[(Item, f64)],
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
    let stored = stored_items(db, Some(&meal.id)).await?;
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

    let food_ids: Vec<i64> = stored.iter().filter_map(|item| item.food_id).collect();
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
    })
}
