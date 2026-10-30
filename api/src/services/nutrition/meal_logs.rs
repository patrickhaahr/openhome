//! Meal Logs (`CONTEXT.md`, Nutrition): what the user ate, at an explicit time.
//!
//! A Meal Log belongs to the Europe/Copenhagen calendar day of its time, so a backfilled meal
//! lands on the day it was eaten. Each Portion copies every known per-100 g value of its Food when
//! it is logged, tagged with where the value came from (ADR 0012): a later Food database import
//! never changes a past Meal Log, and replacing a Portion is the only way to correct one.
//!
//! Logging follows the `request_id` receipt pattern: a retry replays the original result, the
//! same `request_id` with changed arguments is a conflict, and a deleted Meal Log keeps its
//! receipt so an old retry cannot recreate it.

use std::collections::HashMap;

use chrono::{DateTime, TimeDelta, Utc};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use uuid::Uuid;

use super::food_database::{FoodDatabase, FoodError, FoodSnapshot};
use super::parameters::{self, Unit};
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

/// One Portion to log: edible grams of a Food.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PortionRequest {
    pub food_id: i64,
    pub grams: f64,
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
}

/// Omitted fields keep their value; `note: null` clears the note. `portions` replaces every
/// Portion with new snapshots from the current Food database.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EditMealLog {
    #[serde(default)]
    pub eaten_at: Patch<String>,
    #[serde(default)]
    pub note: Patch<String>,
    #[serde(default)]
    pub portions: Patch<Vec<PortionRequest>>,
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

#[derive(Debug, Serialize)]
pub struct Portion {
    pub food_id: i64,
    pub name_da: String,
    pub name_en: String,
    pub grams: f64,
    /// The Food database version the values were copied from.
    pub dataset_version: String,
    pub nutrients: KeyNutrients<PortionAmount>,
}

#[derive(Debug, Serialize)]
pub struct MealLog {
    pub id: String,
    /// When it was eaten, in Copenhagen time with its offset.
    pub eaten_at: String,
    /// The Europe/Copenhagen calendar day it counts toward.
    pub day: String,
    pub note: Option<String>,
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

/// A stored Meal Log with its Portions in order.
#[derive(Debug)]
pub(super) struct StoredLog {
    id: String,
    eaten_at: String,
    day: String,
    note: Option<String>,
    pub portions: Vec<StoredPortion>,
}

/// The arguments a receipt compares on retry.
#[derive(Serialize)]
struct Normalized<'a> {
    eaten_at: &'a str,
    note: Option<&'a str>,
    portions: Vec<(i64, f64)>,
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

fn portions(requests: &[PortionRequest]) -> Result<()> {
    if requests.is_empty() || requests.len() > MAX_PORTIONS {
        return Err(MealLogError::Validation(format!(
            "portions must list 1-{MAX_PORTIONS} Portions"
        )));
    }
    for portion in requests {
        if portion.food_id <= 0 {
            return Err(MealLogError::Validation(
                "food_id must be a positive whole number from search_foods".into(),
            ));
        }
        if !portion.grams.is_finite() || portion.grams <= 0.0 || portion.grams > MAX_GRAMS {
            return Err(MealLogError::Validation(format!(
                "grams must be edible grams above 0 and at most {MAX_GRAMS}"
            )));
        }
    }
    Ok(())
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

/// Snapshots the Foods of `requests` from the current Food database.
async fn snapshots(
    foods: &FoodDatabase,
    requests: &[PortionRequest],
) -> Result<HashMap<i64, FoodSnapshot>> {
    let ids: Vec<i64> = requests.iter().map(|portion| portion.food_id).collect();
    Ok(foods.snapshots(&ids).await?)
}

/// Writes `requests` as the Meal Log's Portions, each with its Food's snapshot.
async fn insert_portions(
    transaction: &mut sqlx::SqliteConnection,
    meal_log_id: &str,
    requests: &[PortionRequest],
    snapshots: &HashMap<i64, FoodSnapshot>,
) -> Result<()> {
    for (position, portion) in requests.iter().enumerate() {
        let snapshot = snapshots
            .get(&portion.food_id)
            .ok_or(FoodError::NotFound(portion.food_id))?;
        let position = position as i64;
        let portion_id = sqlx::query_scalar!(
            "INSERT INTO meal_log_portions (meal_log_id, position, grams, food_id, name_da, name_en, dataset_version) VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING id",
            meal_log_id, position, portion.grams, snapshot.food_id, snapshot.name_da, snapshot.name_en, snapshot.dataset_version)
            .fetch_one(&mut *transaction)
            .await?;
        let values = serde_json::to_string(&snapshot.per_100g)
            .map_err(|error| sqlx::Error::Encode(Box::new(error)))?;
        let source = ValueSource::Food;
        sqlx::query!(
            "INSERT INTO meal_log_portion_values (portion_id, key, value, unit, source) SELECT ?, json_extract(value, '$.key'), json_extract(value, '$.value'), json_extract(value, '$.unit'), ? FROM json_each(?)",
            portion_id, source, values)
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
        r#"SELECT id AS "id!", eaten_at, day, note FROM meal_logs
        WHERE (?1 IS NULL OR id = ?1) AND (?2 IS NULL OR day >= ?2) AND (?3 IS NULL OR day <= ?3)
        ORDER BY eaten_at, created_at, id"#,
        id,
        from,
        to
    )
    .fetch_all(db)
    .await?;
    let portions = sqlx::query!(
        r#"SELECT p.id AS "id!", p.meal_log_id, p.grams, p.food_id, p.name_da, p.name_en, p.dataset_version
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

/// Logs a meal: its Portions with snapshots of their Foods, at its time, with an immutable
/// receipt for its `request_id`. Rejected requests consume no `request_id`.
pub async fn log_meal(db: &SqlitePool, foods: &FoodDatabase, request: LogMeal) -> Result<Logged> {
    let request_id = request_id(&request.request_id)?;
    let at = eaten_at(&request.eaten_at, Utc::now())?;
    let note = note(request.note)?;
    portions(&request.portions)?;
    let stored_at = utc_text(at);
    let normalized = serde_json::to_string(&Normalized {
        eaten_at: &stored_at,
        note: note.as_deref(),
        portions: request
            .portions
            .iter()
            .map(|portion| (portion.food_id, portion.grams))
            .collect(),
    })
    .map_err(|error| sqlx::Error::Encode(Box::new(error)))?;
    if let Some(logged) = replay(db, &request_id, &normalized).await? {
        return Ok(logged);
    }
    let snapshots = snapshots(foods, &request.portions).await?;

    let id = Uuid::new_v4().to_string();
    let day = CalendarDay::of(at).to_string();
    let mut transaction = db.begin().await?;
    // Claim first so simultaneous retries serialize on the receipt.
    let claimed = sqlx::query_scalar!(
        "INSERT INTO meal_log_receipts (request_id, meal_log_id, request) VALUES (?, ?, ?) ON CONFLICT (request_id) DO NOTHING RETURNING request_id",
        request_id, id, normalized)
        .fetch_optional(&mut *transaction)
        .await?;
    if claimed.is_none() {
        transaction.rollback().await?;
        return replay(db, &request_id, &normalized).await?.ok_or_else(|| {
            MealLogError::Conflict(
                "Retry receipt unavailable; list meal_logs before retrying".into(),
            )
        });
    }
    sqlx::query!(
        "INSERT INTO meal_logs (id, eaten_at, day, note) VALUES (?, ?, ?, ?)",
        id,
        stored_at,
        day,
        note
    )
    .execute(&mut *transaction)
    .await?;
    insert_portions(&mut transaction, &id, &request.portions, &snapshots).await?;
    transaction.commit().await?;
    Ok(Logged {
        meal_log: find(db, &id).await?.ok_or_else(not_found)?,
        replayed: false,
    })
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

/// Applies the supplied fields together. Replacing Portions snapshots their Foods afresh.
pub async fn update_meal_log(
    db: &SqlitePool,
    foods: &FoodDatabase,
    id: &str,
    edit: EditMealLog,
) -> Result<MealLog> {
    let at = required(edit.eaten_at, "eaten_at")?
        .map(|value| eaten_at(&value, Utc::now()))
        .transpose()?;
    let note_supplied = edit.note.supplied();
    let note = note(edit.note.value())?;
    let replacement = required(edit.portions, "portions")?;
    if let Some(requests) = &replacement {
        portions(requests)?;
    }
    if find(db, id).await?.is_none() {
        return Err(not_found());
    }
    let snapshots = match &replacement {
        Some(requests) => Some(snapshots(foods, requests).await?),
        None => None,
    };
    let stored_at = at.map(utc_text);
    let day = at.map(|at| CalendarDay::of(at).to_string());
    let mut transaction = db.begin().await?;
    let updated = sqlx::query_scalar!(
        r#"UPDATE meal_logs SET eaten_at = COALESCE(?, eaten_at), day = COALESCE(?, day),
        note = CASE WHEN ? THEN ? ELSE note END,
        updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE id = ? RETURNING id AS "id!""#,
        stored_at,
        day,
        note_supplied,
        note,
        id
    )
    .fetch_optional(&mut *transaction)
    .await?;
    if updated.is_none() {
        return Err(not_found());
    }
    if let (Some(requests), Some(snapshots)) = (&replacement, &snapshots) {
        sqlx::query!("DELETE FROM meal_log_portions WHERE meal_log_id = ?", id)
            .execute(&mut *transaction)
            .await?;
        insert_portions(&mut transaction, id, requests, snapshots).await?;
    }
    transaction.commit().await?;
    find(db, id).await?.ok_or_else(not_found)
}

/// Deletes a Meal Log and its Portions. Its receipt stays, so an old retry cannot recreate it.
pub async fn delete_meal_log(db: &SqlitePool, id: &str) -> Result<()> {
    let deleted = sqlx::query!("DELETE FROM meal_logs WHERE id = ?", id)
        .execute(db)
        .await?;
    if deleted.rows_affected() == 0 {
        return Err(not_found());
    }
    Ok(())
}
