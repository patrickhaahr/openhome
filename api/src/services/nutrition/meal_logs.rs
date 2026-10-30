//! Meal Logs (`CONTEXT.md`, Nutrition): what the user ate, at an explicit time.
//!
//! A Meal Log belongs to the Europe/Copenhagen calendar day of its time, so a backfilled meal
//! lands on the day it was eaten. Each Portion copies every known per-100 g value when it is
//! logged, tagged with where the value came from (ADR 0012): a Product Portion takes its label
//! values first and its linked Food's values for the rest, and the agent may estimate values no
//! source has. A later Food database import or re-link never changes a past Meal Log, and
//! replacing a Portion is the only way to correct one.
//!
//! Logging follows the `request_id` receipt pattern: a retry replays the original result, the
//! same `request_id` with changed arguments is a conflict, and a deleted Meal Log keeps its
//! receipt so an old retry cannot recreate it.
//!
//! An optional photo is encoded before anything is written and stored before the Meal Log row
//! that references it, so a rejected photo writes nothing and a failed write removes its files.

use std::collections::{BTreeMap, HashMap};

use chrono::{DateTime, TimeDelta, Utc};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use uuid::Uuid;

use super::food_database::{FoodDatabase, FoodError};
use super::meal_photos::{MealPhotos, PhotoError, Size, Upload};
use super::parameters::{self, Unit};
use super::products::{self, Barcode, ProductError};
use crate::services::patch::Patch;
use crate::services::training::calendar::{self, CalendarDay};

pub const MAX_PORTIONS: usize = 50;
pub const MAX_GRAMS: f64 = 5_000.0;
pub const MAX_NOTE_CHARS: usize = 500;
/// The longest range one `meal_logs` read covers, in calendar days.
pub const MAX_RANGE_DAYS: i64 = 90;
const MAX_REQUEST_ID_CHARS: usize = 128;
/// How far past the API's clock an eaten-at time may lie, for clock drift between client and API.
const FUTURE_TOLERANCE_MINUTES: i64 = 5;

#[derive(Debug, thiserror::Error)]
pub enum MealLogError {
    #[error("{0}")]
    Validation(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Conflict(String),
    #[error(transparent)]
    Food(#[from] FoodError),
    #[error(transparent)]
    Photo(#[from] PhotoError),
    #[error("Meal Log database operation failed")]
    Database(#[from] sqlx::Error),
}

type Result<T> = std::result::Result<T, MealLogError>;

/// Where a snapshotted value came from: the food database, a Product label or an agent estimate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(rename_all = "snake_case")]
pub enum ValueSource {
    Food,
    Label,
    Estimate,
}

/// One Portion to log: edible grams of a Food (`food_id`) or of a linked Product (`barcode`).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PortionRequest {
    #[serde(default)]
    pub food_id: Option<i64>,
    #[serde(default)]
    pub barcode: Option<String>,
    pub grams: f64,
    /// Agent estimates per 100 g edible part, keyed and united as in the parameter vocabulary,
    /// for values neither a label nor the Food has.
    #[serde(default)]
    pub estimates: BTreeMap<String, f64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LogMeal {
    /// RFC 3339 with an offset: when the meal was eaten.
    pub eaten_at: String,
    #[serde(default)]
    pub note: Option<String>,
    pub portions: Vec<PortionRequest>,
    pub request_id: String,
    /// A base64 JPEG or PNG of the meal.
    #[serde(default)]
    pub photo: Option<String>,
}

/// Omitted fields keep their value; `note: null` clears the note and `photo: null` removes the
/// photo. `portions` replaces every Portion with new snapshots of the current Products and Food
/// database;
/// `photo` (base64 JPEG or PNG) replaces the photo.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EditMealLog {
    #[serde(default)]
    pub eaten_at: Patch<String>,
    #[serde(default)]
    pub note: Patch<String>,
    #[serde(default)]
    pub portions: Patch<Vec<PortionRequest>>,
    #[serde(default)]
    pub photo: Patch<String>,
}

/// One day, or an inclusive range of days.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MealLogsQuery {
    pub day: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
}

/// The energy and macronutrient keys every Portion and total carries; the full set is in
/// `nutrition_day`.
pub const KEY_NUTRIENTS: [&str; 6] = [
    parameters::ENERGY_KCAL,
    parameters::PROTEIN,
    parameters::AVAILABLE_CARBOHYDRATE,
    parameters::FAT,
    parameters::DIETARY_FIBRE,
    parameters::ALCOHOL,
];

/// Energy and macronutrients, in a fixed order.
#[derive(Debug, Serialize)]
pub struct KeyNutrients<T> {
    pub energy_kcal: T,
    pub protein: T,
    pub available_carbohydrate: T,
    pub fat: T,
    pub dietary_fibre: T,
    pub alcohol: T,
}

impl<T> KeyNutrients<T> {
    pub(super) fn from_fn(mut value: impl FnMut(&'static str) -> T) -> Self {
        Self {
            energy_kcal: value(parameters::ENERGY_KCAL),
            protein: value(parameters::PROTEIN),
            available_carbohydrate: value(parameters::AVAILABLE_CARBOHYDRATE),
            fat: value(parameters::FAT),
            dietary_fibre: value(parameters::DIETARY_FIBRE),
            alcohol: value(parameters::ALCOHOL),
        }
    }
}

/// A Portion's amount of one nutrient: its per-100 g value scaled to the grams eaten. `value` and
/// `source` are null when the value is unknown, never zero.
#[derive(Debug, Serialize)]
pub struct PortionAmount {
    pub value: Option<f64>,
    pub unit: Option<&'static str>,
    pub source: Option<ValueSource>,
}

/// A sum over Portions. `value` adds the Portions that have a value; `unknown_portions` counts
/// those that do not. `value` is null when no Portion has one, including when none was eaten.
#[derive(Debug, Serialize)]
pub struct Total {
    pub value: Option<f64>,
    pub unit: Option<String>,
    pub unknown_portions: usize,
}

/// The Product a Portion was logged as, as it was named then.
#[derive(Clone, Debug, Serialize)]
pub struct PortionProduct {
    pub barcode: String,
    pub name: Option<String>,
    pub brand: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Portion {
    /// The Food, or for a Product the Food it was linked to when logged.
    pub food_id: i64,
    /// Null for a Food Portion.
    pub product: Option<PortionProduct>,
    pub name_da: String,
    pub name_en: String,
    pub grams: f64,
    /// The Food database version the values were copied from.
    pub dataset_version: String,
    pub nutrients: KeyNutrients<PortionAmount>,
}

/// Where the photo media routes serve stored photos, by photo ID.
pub const PHOTO_PATH: &str = "/api/nutrition/meal-photos";

#[derive(Debug, Serialize)]
pub struct PhotoThumbnail {
    pub width: u32,
    pub height: u32,
    pub url: String,
}

/// A Meal Log's AVIF photo; a new photo gets a new `id`, so its URLs never change content.
#[derive(Debug, Serialize)]
pub struct Photo {
    pub id: String,
    pub width: u32,
    pub height: u32,
    pub url: String,
    pub thumbnail: PhotoThumbnail,
}

#[derive(Debug, Serialize)]
pub struct MealLog {
    pub id: String,
    /// When it was eaten, in Copenhagen time with its offset.
    pub eaten_at: String,
    /// The Europe/Copenhagen calendar day it counts toward.
    pub day: String,
    pub note: Option<String>,
    pub photo: Option<Photo>,
    pub portions: Vec<Portion>,
    pub totals: KeyNutrients<Total>,
}

#[derive(Debug, Serialize)]
pub struct Logged {
    pub meal_log: MealLog,
    pub replayed: bool,
}

#[derive(Debug, Serialize)]
pub struct MealLogs {
    pub from: String,
    pub to: String,
    pub time_zone: &'static str,
    /// Oldest first.
    pub meal_logs: Vec<MealLog>,
}

/// A snapshotted value: per 100 g edible part.
#[derive(Debug)]
pub(super) struct StoredValue {
    pub value: f64,
    pub unit: String,
    pub source: ValueSource,
}

/// A stored Portion with its snapshot.
#[derive(Debug)]
pub(super) struct StoredPortion {
    food_id: i64,
    product: Option<PortionProduct>,
    name_da: String,
    name_en: String,
    pub grams: f64,
    dataset_version: String,
    pub values: HashMap<String, StoredValue>,
}

impl StoredPortion {
    /// The amount eaten of `key`, if known: the per-100 g value scaled to the grams.
    pub(super) fn amount(&self, key: &str) -> Option<f64> {
        self.values
            .get(key)
            .map(|stored| stored.value * self.grams / 100.0)
    }
}

/// A stored photo: its ID names its files.
#[derive(Debug)]
struct StoredPhoto {
    id: String,
    size: Size,
    thumbnail_size: Size,
}

impl StoredPhoto {
    /// The photo when every column is set; the columns are written together.
    fn from_columns(
        id: Option<String>,
        width: Option<i64>,
        height: Option<i64>,
        thumbnail_width: Option<i64>,
        thumbnail_height: Option<i64>,
    ) -> Option<Self> {
        let side = |value: Option<i64>| value.and_then(|value| u32::try_from(value).ok());
        Some(Self {
            id: id?,
            size: Size {
                width: side(width)?,
                height: side(height)?,
            },
            thumbnail_size: Size {
                width: side(thumbnail_width)?,
                height: side(thumbnail_height)?,
            },
        })
    }

    fn into_photo(self) -> Photo {
        let url = format!("{PHOTO_PATH}/{}", self.id);
        Photo {
            thumbnail: PhotoThumbnail {
                width: self.thumbnail_size.width,
                height: self.thumbnail_size.height,
                url: format!("{url}/thumbnail"),
            },
            id: self.id,
            width: self.size.width,
            height: self.size.height,
            url,
        }
    }
}

/// A stored Meal Log with its Portions in order.
#[derive(Debug)]
pub(super) struct StoredLog {
    id: String,
    eaten_at: String,
    day: String,
    note: Option<String>,
    photo: Option<StoredPhoto>,
    pub portions: Vec<StoredPortion>,
}

/// The arguments a receipt compares on retry. `photo` is the upload's SHA-256, left out when
/// there is none so receipts written before photos existed still match their retries.
#[derive(Serialize)]
struct Normalized<'a> {
    eaten_at: &'a str,
    note: Option<&'a str>,
    /// `[food_id, grams]` for a Food without estimates, as receipts were first written;
    /// otherwise an object.
    portions: Vec<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    photo: Option<String>,
}

/// Rounds away floating-point noise from scaled and summed values.
pub(super) fn round(value: f64) -> f64 {
    (value * 10_000.0).round() / 10_000.0
}

fn vocabulary_unit(key: &str) -> Option<&'static str> {
    parameters::by_key(key).and_then(|parameter| Unit::symbol(parameter.unit))
}

/// Sums `key` over `portions`, keeping unknown values out of the sum and counting them.
pub(super) fn total(key: &str, unit: Option<String>, portions: &[&StoredPortion]) -> Total {
    let mut value: Option<f64> = None;
    let mut unknown_portions = 0;
    for portion in portions {
        match portion.amount(key) {
            Some(amount) => *value.get_or_insert(0.0) += amount,
            None => unknown_portions += 1,
        }
    }
    Total {
        value: value.map(round),
        unit,
        unknown_portions,
    }
}

pub(super) fn key_totals(portions: &[&StoredPortion]) -> KeyNutrients<Total> {
    KeyNutrients::from_fn(|key| total(key, vocabulary_unit(key).map(str::to_owned), portions))
}

impl StoredLog {
    pub(super) fn into_meal_log(self) -> Result<MealLog> {
        let eaten_at = DateTime::parse_from_rfc3339(&self.eaten_at)
            .map_err(|error| sqlx::Error::Decode(Box::new(error)))?
            .with_timezone(&Utc);
        let totals = key_totals(&self.portions.iter().collect::<Vec<_>>());
        Ok(MealLog {
            id: self.id,
            eaten_at: calendar::iso(eaten_at),
            day: self.day,
            note: self.note,
            photo: self.photo.map(StoredPhoto::into_photo),
            portions: self
                .portions
                .into_iter()
                .map(|portion| Portion {
                    nutrients: KeyNutrients::from_fn(|key| {
                        let stored = portion.values.get(key);
                        PortionAmount {
                            value: portion.amount(key).map(round),
                            unit: vocabulary_unit(key),
                            source: stored.map(|stored| stored.source),
                        }
                    }),
                    food_id: portion.food_id,
                    product: portion.product,
                    name_da: portion.name_da,
                    name_en: portion.name_en,
                    grams: portion.grams,
                    dataset_version: portion.dataset_version,
                })
                .collect(),
            totals,
        })
    }
}

fn request_id(value: &str) -> Result<String> {
    let value = value.trim();
    if value.is_empty()
        || value.chars().count() > MAX_REQUEST_ID_CHARS
        || value.chars().any(char::is_control)
    {
        return Err(MealLogError::Validation(format!(
            "request_id must contain 1-{MAX_REQUEST_ID_CHARS} characters without control characters"
        )));
    }
    Ok(value.to_owned())
}

/// Parses an eaten-at time to whole seconds. Past times are backfills; the future is refused.
fn eaten_at(value: &str, now: DateTime<Utc>) -> Result<DateTime<Utc>> {
    let at = DateTime::parse_from_rfc3339(value.trim())
        .ok()
        .and_then(|at| DateTime::from_timestamp(at.timestamp(), 0))
        .ok_or_else(|| {
            MealLogError::Validation(
                "eaten_at must be an RFC 3339 timestamp with an offset, e.g. \
                 2026-10-30T08:15:00+01:00"
                    .into(),
            )
        })?;
    if at > now + TimeDelta::minutes(FUTURE_TOLERANCE_MINUTES) {
        return Err(MealLogError::Validation(
            "eaten_at is in the future; log a meal once it has been eaten".into(),
        ));
    }
    if CalendarDay::parse(&CalendarDay::of(at).to_string()).is_none() {
        return Err(MealLogError::Validation(
            "eaten_at is outside the supported range".into(),
        ));
    }
    Ok(at)
}

fn utc_text(at: DateTime<Utc>) -> String {
    at.format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

/// A note trimmed of surrounding whitespace; empty is no note.
fn note(value: Option<String>) -> Result<Option<String>> {
    let Some(value) = value else {
        return Ok(None);
    };
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    if value.chars().count() > MAX_NOTE_CHARS
        || value
            .chars()
            .any(|character| character.is_control() && character != '\n')
    {
        return Err(MealLogError::Validation(format!(
            "note must be at most {MAX_NOTE_CHARS} characters without control characters"
        )));
    }
    Ok(Some(value.to_owned()))
}

/// What a Portion is of.
enum Item {
    Food(i64),
    Product(Barcode),
}

/// A validated Portion request.
struct ValidPortion {
    item: Item,
    grams: f64,
    estimates: BTreeMap<String, f64>,
}

impl ValidPortion {
    /// The receipt form: `[food_id, grams]` for a Food without estimates, as receipts were
    /// first written, so their retries still match.
    fn normalized(&self) -> serde_json::Value {
        match &self.item {
            Item::Food(food_id) if self.estimates.is_empty() => {
                serde_json::json!([food_id, self.grams])
            }
            Item::Food(food_id) => serde_json::json!({
                "food_id": food_id, "grams": self.grams, "estimates": self.estimates,
            }),
            Item::Product(barcode) => serde_json::json!({
                "barcode": barcode.as_str(), "grams": self.grams, "estimates": self.estimates,
            }),
        }
    }
}

/// The most energy 100 g can hold: pure fat.
const MAX_KCAL_PER_100G: f64 = 900.0;
const MAX_KJ_PER_100G: f64 = 3_800.0;

/// Checks an estimate: a per-100 g vocabulary key with a plausible, non-negative value.
fn estimate(key: &str, value: f64) -> Result<()> {
    let invalid = |reason: &str| {
        Err(MealLogError::Validation(format!(
            "estimates.{key} {reason}; estimates are per 100 g edible part, keyed and united as in \
             the Food database's parameters"
        )))
    };
    let Some(parameter) = parameters::by_key(key).filter(|parameter| parameter.unit.per_100g())
    else {
        return invalid("is not a per-100 g parameter key");
    };
    let limit = match parameter.unit {
        Unit::Kilocalorie => MAX_KCAL_PER_100G,
        Unit::Kilojoule => MAX_KJ_PER_100G,
        unit => unit
            .per_gram()
            .map_or(f64::MAX, |per_gram| 100.0 * per_gram),
    };
    if !value.is_finite() || value < 0.0 || value > limit {
        return invalid(&format!("must be between 0 and {limit}"));
    }
    Ok(())
}

fn portions(requests: Vec<PortionRequest>) -> Result<Vec<ValidPortion>> {
    if requests.is_empty() || requests.len() > MAX_PORTIONS {
        return Err(MealLogError::Validation(format!(
            "portions must list 1-{MAX_PORTIONS} Portions"
        )));
    }
    requests
        .into_iter()
        .map(|portion| {
            let item = match (portion.food_id, portion.barcode) {
                (Some(food_id), None) if food_id > 0 => Item::Food(food_id),
                (Some(_), None) => {
                    return Err(MealLogError::Validation(
                        "food_id must be a positive whole number from search_foods".into(),
                    ));
                }
                (None, Some(barcode)) => {
                    Item::Product(Barcode::parse(&barcode).map_err(|error| match error {
                        ProductError::Validation(message) => MealLogError::Validation(message),
                        other => MealLogError::Validation(other.to_string()),
                    })?)
                }
                _ => {
                    return Err(MealLogError::Validation(
                        "each Portion needs either a food_id or the barcode of a linked Product"
                            .into(),
                    ));
                }
            };
            if !portion.grams.is_finite() || portion.grams <= 0.0 || portion.grams > MAX_GRAMS {
                return Err(MealLogError::Validation(format!(
                    "grams must be edible grams above 0 and at most {MAX_GRAMS}"
                )));
            }
            for (key, value) in &portion.estimates {
                estimate(key, *value)?;
            }
            Ok(ValidPortion {
                item,
                grams: portion.grams,
                estimates: portion.estimates,
            })
        })
        .collect()
}

fn required<T>(patch: Patch<T>, field: &str) -> Result<Option<T>> {
    match patch {
        Patch::Missing => Ok(None),
        Patch::Value(Some(value)) => Ok(Some(value)),
        Patch::Value(None) => Err(MealLogError::Validation(format!("{field} cannot be null"))),
    }
}

/// The calendar day `text` names.
pub fn parse_day(text: &str, field: &str) -> Result<CalendarDay> {
    CalendarDay::parse(text).ok_or_else(|| {
        MealLogError::Validation(format!(
            "{field} must be a Europe/Copenhagen calendar date written YYYY-MM-DD"
        ))
    })
}

/// One snapshotted per-100 g value with its source.
#[derive(Serialize)]
struct PreparedValue {
    key: String,
    value: f64,
    unit: String,
    source: ValueSource,
}

/// A Portion ready to write: its Food, its Product if any, and its snapshot.
struct PreparedPortion {
    grams: f64,
    food_id: i64,
    name_da: String,
    name_en: String,
    dataset_version: String,
    product: Option<PortionProduct>,
    values: Vec<PreparedValue>,
}

/// Snapshots each Portion: a Product's label values, then its Food's values for every key the
/// label lacks, then the estimates, which may only fill keys neither has.
async fn prepare(
    db: &SqlitePool,
    foods: &FoodDatabase,
    portions: &[ValidPortion],
) -> Result<Vec<PreparedPortion>> {
    let barcodes: Vec<&Barcode> = portions
        .iter()
        .filter_map(|portion| match &portion.item {
            Item::Product(barcode) => Some(barcode),
            Item::Food(_) => None,
        })
        .collect();
    let stored = products::stored(db, &barcodes).await?;
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
    let snapshots = foods.snapshots(&food_ids).await?;
    portions
        .iter()
        .zip(items)
        .map(|(portion, (food_id, product))| {
            let food = snapshots
                .get(&food_id)
                .ok_or(FoodError::NotFound(food_id))?;
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

/// Writes the Meal Log's Portions, each with its snapshot.
async fn insert_portions(
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
        let portion_id = sqlx::query_scalar!(
            "INSERT INTO meal_log_portions (meal_log_id, position, grams, food_id, name_da, name_en, dataset_version, product_barcode, product_name, product_brand) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
            meal_log_id, position, portion.grams, portion.food_id, portion.name_da, portion.name_en, portion.dataset_version, barcode, name, brand)
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

/// Which Meal Logs to load.
pub(super) enum Selection<'a> {
    One(&'a str),
    Days(CalendarDay, CalendarDay),
}

/// Loads Meal Logs in time order with their Portions; with `all_values` every snapshotted
/// value, otherwise only the key nutrients.
pub(super) async fn load(
    db: &SqlitePool,
    selection: &Selection<'_>,
    all_values: bool,
) -> Result<Vec<StoredLog>> {
    let (id, from, to) = match selection {
        Selection::One(id) => (Some((*id).to_owned()), None, None),
        Selection::Days(from, to) => (None, Some(from.to_string()), Some(to.to_string())),
    };
    let logs = sqlx::query!(
        r#"SELECT id AS "id!", eaten_at, day, note, photo_id, photo_width, photo_height,
            photo_thumbnail_width, photo_thumbnail_height FROM meal_logs
        WHERE (?1 IS NULL OR id = ?1) AND (?2 IS NULL OR day >= ?2) AND (?3 IS NULL OR day <= ?3)
        ORDER BY eaten_at, created_at, id"#,
        id,
        from,
        to
    )
    .fetch_all(db)
    .await?;
    let portions = sqlx::query!(
        r#"SELECT p.id AS "id!", p.meal_log_id, p.grams, p.food_id, p.name_da, p.name_en, p.dataset_version,
            p.product_barcode, p.product_name, p.product_brand
        FROM meal_log_portions p JOIN meal_logs m ON m.id = p.meal_log_id
        WHERE (?1 IS NULL OR m.id = ?1) AND (?2 IS NULL OR m.day >= ?2) AND (?3 IS NULL OR m.day <= ?3)
        ORDER BY p.meal_log_id, p.position"#,
        id,
        from,
        to
    )
    .fetch_all(db)
    .await?;
    let values = sqlx::query!(
        r#"SELECT v.portion_id, v.key, v.value, v.unit, v.source AS "source: ValueSource"
        FROM meal_log_portion_values v
        JOIN meal_log_portions p ON p.id = v.portion_id JOIN meal_logs m ON m.id = p.meal_log_id
        WHERE (?1 IS NULL OR m.id = ?1) AND (?2 IS NULL OR m.day >= ?2) AND (?3 IS NULL OR m.day <= ?3)
            AND (?4 OR v.key IN ('energy_kcal', 'protein', 'available_carbohydrate', 'fat', 'dietary_fibre', 'alcohol'))"#,
        id,
        from,
        to,
        all_values
    )
    .fetch_all(db)
    .await?;

    let mut values_by_portion: HashMap<i64, HashMap<String, StoredValue>> = HashMap::new();
    for row in values {
        values_by_portion.entry(row.portion_id).or_default().insert(
            row.key,
            StoredValue {
                value: row.value,
                unit: row.unit,
                source: row.source,
            },
        );
    }
    let mut portions_by_log: HashMap<String, Vec<StoredPortion>> = HashMap::new();
    for row in portions {
        portions_by_log
            .entry(row.meal_log_id)
            .or_default()
            .push(StoredPortion {
                food_id: row.food_id,
                product: row.product_barcode.map(|barcode| PortionProduct {
                    barcode,
                    name: row.product_name,
                    brand: row.product_brand,
                }),
                name_da: row.name_da,
                name_en: row.name_en,
                grams: row.grams,
                dataset_version: row.dataset_version,
                values: values_by_portion.remove(&row.id).unwrap_or_default(),
            });
    }
    Ok(logs
        .into_iter()
        .map(|row| StoredLog {
            portions: portions_by_log.remove(&row.id).unwrap_or_default(),
            photo: StoredPhoto::from_columns(
                row.photo_id,
                row.photo_width,
                row.photo_height,
                row.photo_thumbnail_width,
                row.photo_thumbnail_height,
            ),
            id: row.id,
            eaten_at: row.eaten_at,
            day: row.day,
            note: row.note,
        })
        .collect())
}

/// One Meal Log, or `None` when it does not exist.
async fn find(db: &SqlitePool, id: &str) -> Result<Option<MealLog>> {
    load(db, &Selection::One(id), false)
        .await?
        .into_iter()
        .next()
        .map(StoredLog::into_meal_log)
        .transpose()
}

fn not_found() -> MealLogError {
    MealLogError::NotFound("Meal Log not found; list meal_logs for its id".into())
}

async fn replay(db: &SqlitePool, request_id: &str, request: &str) -> Result<Option<Logged>> {
    let Some(receipt) = sqlx::query!(
        "SELECT meal_log_id, request FROM meal_log_receipts WHERE request_id = ?",
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
    let meal_log = find(db, &receipt.meal_log_id).await?.ok_or_else(|| {
        MealLogError::Conflict(
            "The retried Meal Log was deleted; use a new request_id to log it again".into(),
        )
    })?;
    Ok(Some(Logged {
        meal_log,
        replayed: true,
    }))
}

/// The photo columns of a Meal Log row: all set for a stored photo, all null without one.
#[derive(Default)]
struct PhotoColumns {
    id: Option<String>,
    width: Option<i64>,
    height: Option<i64>,
    thumbnail_width: Option<i64>,
    thumbnail_height: Option<i64>,
}

impl PhotoColumns {
    fn of(photo: Option<&StoredPhoto>) -> Self {
        photo.map_or_else(Self::default, |photo| Self {
            id: Some(photo.id.clone()),
            width: Some(photo.size.width.into()),
            height: Some(photo.size.height.into()),
            thumbnail_width: Some(photo.thumbnail_size.width.into()),
            thumbnail_height: Some(photo.thumbnail_size.height.into()),
        })
    }
}

/// Encodes and stores `upload`, waiting for any encode already running.
async fn store_photo(photos: &MealPhotos, upload: Upload) -> Result<StoredPhoto> {
    let encoded = photos.encode(upload).await?;
    let id = photos.store(&encoded).await?;
    Ok(StoredPhoto {
        id,
        size: encoded.size,
        thumbnail_size: encoded.thumbnail_size,
    })
}

/// Logs a meal: its Portions with snapshots of their Foods and its optional photo, at its time,
/// with an immutable receipt for its `request_id`. Rejected requests consume no `request_id` and
/// write nothing.
pub async fn log_meal(
    db: &SqlitePool,
    foods: &FoodDatabase,
    photos: &MealPhotos,
    request: LogMeal,
) -> Result<Logged> {
    let request_id = request_id(&request.request_id)?;
    let at = eaten_at(&request.eaten_at, Utc::now())?;
    let note = note(request.note)?;
    let requests = portions(request.portions)?;
    let upload = request
        .photo
        .as_deref()
        .map(Upload::from_base64)
        .transpose()?;
    let stored_at = utc_text(at);
    let normalized = serde_json::to_string(&Normalized {
        eaten_at: &stored_at,
        note: note.as_deref(),
        portions: requests.iter().map(ValidPortion::normalized).collect(),
        photo: upload.as_ref().map(Upload::sha256),
    })
    .map_err(|error| sqlx::Error::Encode(Box::new(error)))?;
    if let Some(logged) = replay(db, &request_id, &normalized).await? {
        return Ok(logged);
    }
    let prepared = prepare(db, foods, &requests).await?;
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
        note: note.as_deref(),
        photo: PhotoColumns::of(photo.as_ref()),
    };
    let inserted = insert_log(db, &row, &prepared).await;
    if !matches!(inserted, Ok(true))
        && let Some(photo) = &photo
    {
        photos.remove(&photo.id).await;
    }
    if !inserted? {
        return replay(db, &request_id, &normalized).await?.ok_or_else(|| {
            MealLogError::Conflict(
                "Retry receipt unavailable; list meal_logs before retrying".into(),
            )
        });
    }
    Ok(Logged {
        meal_log: find(db, &id).await?.ok_or_else(not_found)?,
        replayed: false,
    })
}

/// A Meal Log row to insert with its receipt.
struct NewLog<'a> {
    id: &'a str,
    request_id: &'a str,
    normalized: &'a str,
    eaten_at: &'a str,
    day: String,
    note: Option<&'a str>,
    photo: PhotoColumns,
}

/// Inserts the receipt, Meal Log and Portions together. `false` when a simultaneous retry
/// claimed the receipt first; nothing is written then.
async fn insert_log(
    db: &SqlitePool,
    row: &NewLog<'_>,
    portions: &[PreparedPortion],
) -> Result<bool> {
    let mut transaction = db.begin().await?;
    // Claim first so simultaneous retries serialize on the receipt.
    let claimed = sqlx::query_scalar!(
        "INSERT INTO meal_log_receipts (request_id, meal_log_id, request) VALUES (?, ?, ?) ON CONFLICT (request_id) DO NOTHING RETURNING request_id",
        row.request_id, row.id, row.normalized)
        .fetch_optional(&mut *transaction)
        .await?;
    if claimed.is_none() {
        transaction.rollback().await?;
        return Ok(false);
    }
    let photo = &row.photo;
    sqlx::query!(
        "INSERT INTO meal_logs (id, eaten_at, day, note, photo_id, photo_width, photo_height, photo_thumbnail_width, photo_thumbnail_height) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        row.id, row.eaten_at, row.day, row.note,
        photo.id, photo.width, photo.height, photo.thumbnail_width, photo.thumbnail_height)
    .execute(&mut *transaction)
    .await?;
    insert_portions(&mut transaction, row.id, portions).await?;
    transaction.commit().await?;
    Ok(true)
}

/// Meal Logs of one day, or of an inclusive range of up to [`MAX_RANGE_DAYS`] days, in time
/// order.
pub async fn meal_logs(db: &SqlitePool, query: &MealLogsQuery) -> Result<MealLogs> {
    let (from, to) = match (&query.day, &query.from, &query.to) {
        (Some(day), None, None) => {
            let day = parse_day(day, "day")?;
            (day, day)
        }
        (None, Some(from), Some(to)) => {
            let (from, to) = (parse_day(from, "from")?, parse_day(to, "to")?);
            if !(0..MAX_RANGE_DAYS).contains(&from.days_until(to)) {
                return Err(MealLogError::Validation(format!(
                    "from and to must cover 1 to {MAX_RANGE_DAYS} calendar days, with from <= to"
                )));
            }
            (from, to)
        }
        _ => {
            return Err(MealLogError::Validation(
                "give either day, or both from and to (YYYY-MM-DD)".into(),
            ));
        }
    };
    let meal_logs = load(db, &Selection::Days(from, to), false)
        .await?
        .into_iter()
        .map(StoredLog::into_meal_log)
        .collect::<Result<_>>()?;
    Ok(MealLogs {
        from: from.to_string(),
        to: to.to_string(),
        time_zone: calendar::TIME_ZONE_NAME,
        meal_logs,
    })
}

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
    photo: PhotoChange,
}

/// Applies the supplied fields together. Replacing Portions snapshots their Foods afresh;
/// replacing the photo encodes it before anything is written and removes the replaced files.
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
    let photo = match edit.photo {
        Patch::Missing => PhotoChange::Keep,
        Patch::Value(None) => PhotoChange::Remove,
        Patch::Value(Some(text)) => PhotoChange::Replace(Upload::from_base64(&text)?),
    };
    if find(db, id).await?.is_none() {
        return Err(not_found());
    }
    let portions = match replacement {
        Some(requests) => Some(prepare(db, foods, &requests).await?),
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
        changes.eaten_at,
        changes.note.as_ref(),
        changes.portions.as_ref(),
        photo_supplied.then(|| PhotoColumns::of(photo.as_ref())),
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

/// Writes one edit in a transaction. Returns the ID of the photo it replaced or removed.
async fn write_changes(
    db: &SqlitePool,
    id: &str,
    at: Option<DateTime<Utc>>,
    note: Option<&Option<String>>,
    portions: Option<&Vec<PreparedPortion>>,
    photo: Option<PhotoColumns>,
) -> Result<Option<String>> {
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
