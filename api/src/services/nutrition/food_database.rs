//! The Food database (`food.db`): the Danish Food Composition Database, owned by the API.
//!
//! `food.db` is a third SQLite database beside `app.db` and `noop.db`, opened with the shared
//! settings and migrated by its own embedded migrations (`food_migrations/`). When it does not
//! record the pinned dataset version, [`FoodDatabase::open`] hands back a [`FoodImport`] to run in
//! the background; lookups report unavailable until it completes, and a failed import leaves any
//! previous complete import in place (ADR 0013). It is rebuildable reference data: nothing the
//! user decides is stored here.
//!
//! `food.db` queries use runtime-checked `sqlx::query`: the compile-time macros verify against
//! `DATABASE_URL` (`app.db`), which does not contain this schema.

mod import;
mod search;
mod workbook;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::{Arc, PoisonError, RwLock};

use serde::Serialize;
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::{QueryBuilder, Row, SqlitePool};
use thiserror::Error;

pub use import::{DatasetPin, ImportError};
use search::{FoodIdentity, SearchIndex};

use super::parameters::{self, Unit};

/// File name of the Food database, placed next to the main database by default.
pub const FOOD_DB_FILE_NAME: &str = "food.db";
/// Subdirectory of the data directory holding the verified workbook.
const CACHE_DIR_NAME: &str = "food-dataset";
pub const DEFAULT_SEARCH_LIMIT: usize = 20;
pub const MAX_SEARCH_LIMIT: usize = 50;
const MAX_QUERY_CHARS: usize = 100;

const SOURCE_NAME: &str = "The Danish Food Composition Database";
const SOURCE_LICENCE: &str = "CC BY 4.0";

/// The per-100 g values search results carry.
const KEY_VALUES: [&str; 8] = [
    parameters::ENERGY_KCAL,
    parameters::ENERGY_KJ,
    parameters::PROTEIN,
    parameters::FAT,
    parameters::AVAILABLE_CARBOHYDRATE,
    parameters::SUGARS,
    parameters::DIETARY_FIBRE,
    parameters::ALCOHOL,
];

/// Lookups over `food.db`, available once a complete import of the pinned dataset exists.
#[derive(Clone)]
pub struct FoodDatabase {
    pool: SqlitePool,
    state: Arc<RwLock<State>>,
}

enum State {
    Importing,
    Failed,
    Ready(Arc<Ready>),
}

struct Ready {
    version: String,
    index: SearchIndex,
}

/// Why Food lookups cannot answer yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Unavailable {
    /// The pinned dataset is being downloaded or imported.
    Importing,
    /// The import failed and no earlier complete import exists; the next start retries.
    Failed,
}

#[derive(Debug, Error)]
pub enum FoodError {
    #[error("the food database is unavailable")]
    Unavailable(Unavailable),
    #[error("{0}")]
    Validation(String),
    #[error("Food {0} was not found")]
    NotFound(i64),
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

/// The pending import of the pinned dataset; run it in the background.
pub struct FoodImport {
    database: FoodDatabase,
    pin: DatasetPin,
    cache_dir: PathBuf,
}

/// The dataset values come from, to credit wherever they are shown.
#[derive(Debug, Serialize)]
pub struct Source {
    pub name: &'static str,
    pub version: String,
    pub credit: String,
    pub licence: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct FoodGroup {
    pub id: Option<i64>,
    pub name_da: Option<String>,
    pub name_en: Option<String>,
}

/// A value with its unit; `value` is null when the database has no value, never zero.
#[derive(Debug, Serialize)]
pub struct Amount {
    pub value: Option<f64>,
    pub unit: Option<&'static str>,
}

/// Key values per 100 g edible part.
#[derive(Debug, Serialize)]
pub struct KeyValues {
    pub energy_kcal: Amount,
    pub energy_kj: Amount,
    pub protein: Amount,
    pub fat: Amount,
    pub available_carbohydrate: Amount,
    pub sugars: Amount,
    pub dietary_fibre: Amount,
    pub alcohol: Amount,
}

#[derive(Debug, Serialize)]
pub struct FoodSummary {
    pub food_id: i64,
    pub name_da: String,
    pub name_en: String,
    pub food_group: Option<FoodGroup>,
    pub per_100g: KeyValues,
    /// Grams per millilitre.
    pub density: Amount,
    /// Inedible percentage of the Food as bought.
    pub waste: Amount,
}

#[derive(Debug, Serialize)]
pub struct FoodSearch {
    pub query: String,
    pub foods: Vec<FoodSummary>,
    pub source: Source,
}

#[derive(Clone, Debug, Serialize)]
pub struct ParameterGroup {
    pub name_da: String,
    pub name_en: String,
}

/// One parameter of a Food: per 100 g edible part when `per_100g`, else a property of the Food.
#[derive(Debug, Serialize)]
pub struct ParameterValue {
    pub key: String,
    pub parameter_id: i64,
    pub name_da: String,
    pub name_en: String,
    pub group: ParameterGroup,
    pub unit: Option<String>,
    pub per_100g: bool,
    pub value: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct Food {
    pub food_id: i64,
    pub name_da: String,
    pub name_en: String,
    pub food_group: Option<FoodGroup>,
    pub density: Amount,
    pub waste: Amount,
    /// Every parameter of the dataset in its sort order, including unknown (null) values.
    pub parameters: Vec<ParameterValue>,
    pub source: Source,
}

/// A Food's known per-100 g values, as a Portion copies them when it is logged (ADR 0012).
#[derive(Debug)]
pub struct FoodSnapshot {
    pub food_id: i64,
    pub name_da: String,
    pub name_en: String,
    /// The dataset version the values come from; `FoodIDs` change between versions.
    pub dataset_version: String,
    /// Every known per-100 g value in database order; an unknown value is absent, never zero.
    pub per_100g: Vec<SnapshotValue>,
}

/// One per-100 g value with its vocabulary key and unit symbol.
#[derive(Debug, Serialize)]
pub struct SnapshotValue {
    pub key: String,
    pub value: f64,
    pub unit: String,
}

/// A per-100 g parameter of the imported dataset: what a day's nutrient totals are named by.
#[derive(Debug)]
pub struct NutrientParameter {
    pub key: String,
    pub name_da: String,
    pub name_en: String,
    pub group: ParameterGroup,
    pub unit: Option<String>,
}

/// Resolves the Food database URL: `FOOD_DB_URL` when set, otherwise `DATABASE_URL` with the file
/// name swapped to `food.db` (query parameters are kept).
pub fn resolve_db_url(database_url: &str, food_db_url: Option<&str>) -> anyhow::Result<String> {
    crate::db::sibling_db_url(database_url, food_db_url, FOOD_DB_FILE_NAME).ok_or_else(|| {
        anyhow::anyhow!(
            "cannot derive the food database location from DATABASE_URL; set FOOD_DB_URL explicitly"
        )
    })
}

/// Where the verified workbook is cached: a `food-dataset` directory beside `food.db`.
pub fn cache_dir(food_db_url: &str) -> anyhow::Result<PathBuf> {
    let options = SqliteConnectOptions::from_str(food_db_url)?;
    let file = options.get_filename();
    anyhow::ensure!(
        file != Path::new(":memory:") && !file.as_os_str().is_empty(),
        "the food database must be a file so its workbook can be cached beside it"
    );
    Ok(file
        .parent()
        .unwrap_or_else(|| Path::new(""))
        .join(CACHE_DIR_NAME))
}

impl FoodDatabase {
    /// Migrates `food.db` and checks its recorded dataset against `pin`. Returns the import to run
    /// when the pinned version is not recorded; lookups report importing until it finishes.
    pub async fn open(
        pool: SqlitePool,
        pin: DatasetPin,
        cache_dir: PathBuf,
    ) -> anyhow::Result<(Self, Option<FoodImport>)> {
        sqlx::migrate!("./food_migrations").run(&pool).await?;
        let recorded = sqlx::query("SELECT version, sha256 FROM dataset WHERE id = 1")
            .fetch_optional(&pool)
            .await?
            .map(|row| {
                (
                    row.get::<String, _>("version"),
                    row.get::<String, _>("sha256"),
                )
            });
        if recorded.is_some_and(|(version, sha256)| version == pin.version && sha256 == pin.sha256)
        {
            let ready = load(&pool).await?;
            let database = Self::with_state(pool, ready.map_or(State::Failed, State::Ready));
            return Ok((database, None));
        }
        let database = Self::with_state(pool, State::Importing);
        let import = FoodImport {
            database: database.clone(),
            pin,
            cache_dir,
        };
        Ok((database, Some(import)))
    }

    fn with_state(pool: SqlitePool, state: State) -> Self {
        Self {
            pool,
            state: Arc::new(RwLock::new(state)),
        }
    }

    fn set_state(&self, state: State) {
        *self.state.write().unwrap_or_else(PoisonError::into_inner) = state;
    }

    fn ready(&self) -> Result<Arc<Ready>, FoodError> {
        match &*self.state.read().unwrap_or_else(PoisonError::into_inner) {
            State::Ready(ready) => Ok(Arc::clone(ready)),
            State::Importing => Err(FoodError::Unavailable(Unavailable::Importing)),
            State::Failed => Err(FoodError::Unavailable(Unavailable::Failed)),
        }
    }

    /// Foods whose Danish or English name matches `query`, best first, with key values.
    pub async fn search(&self, query: &str, limit: Option<usize>) -> Result<FoodSearch, FoodError> {
        let query = query.trim();
        if query.is_empty() || !query.chars().any(char::is_alphanumeric) {
            return Err(FoodError::Validation(
                "query must contain a letter or digit".into(),
            ));
        }
        if query.chars().count() > MAX_QUERY_CHARS {
            return Err(FoodError::Validation(format!(
                "query must be at most {MAX_QUERY_CHARS} characters"
            )));
        }
        let limit = limit.unwrap_or(DEFAULT_SEARCH_LIMIT);
        if !(1..=MAX_SEARCH_LIMIT).contains(&limit) {
            return Err(FoodError::Validation(format!(
                "limit must be between 1 and {MAX_SEARCH_LIMIT}"
            )));
        }
        let ready = self.ready()?;
        let found = ready.index.search(query, limit);
        let mut values = self.summary_values(&found).await?;
        let foods = found
            .into_iter()
            .map(|food| {
                let mut values = values.remove(&food.food_id).unwrap_or_default();
                let mut take = |key: &'static str| amount(key, values.remove(key));
                FoodSummary {
                    food_id: food.food_id,
                    name_da: food.name_da.clone(),
                    name_en: food.name_en.clone(),
                    food_group: food.food_group.clone(),
                    per_100g: KeyValues {
                        energy_kcal: take(parameters::ENERGY_KCAL),
                        energy_kj: take(parameters::ENERGY_KJ),
                        protein: take(parameters::PROTEIN),
                        fat: take(parameters::FAT),
                        available_carbohydrate: take(parameters::AVAILABLE_CARBOHYDRATE),
                        sugars: take(parameters::SUGARS),
                        dietary_fibre: take(parameters::DIETARY_FIBRE),
                        alcohol: take(parameters::ALCOHOL),
                    },
                    density: take(parameters::DENSITY),
                    waste: take(parameters::WASTE),
                }
            })
            .collect();
        Ok(FoodSearch {
            query: query.to_owned(),
            foods,
            source: source(&ready.version),
        })
    }

    /// The key, density and waste values of the found Foods, by `FoodID` and key.
    async fn summary_values(
        &self,
        foods: &[&FoodIdentity],
    ) -> Result<HashMap<i64, HashMap<String, f64>>, sqlx::Error> {
        let mut values: HashMap<i64, HashMap<String, f64>> = HashMap::new();
        if foods.is_empty() {
            return Ok(values);
        }
        let mut query = QueryBuilder::new(
            "SELECT v.food_id, p.key, v.value FROM food_value v \
             JOIN parameter p ON p.parameter_id = v.parameter_id WHERE v.food_id IN (",
        );
        let mut ids = query.separated(", ");
        for food in foods {
            ids.push_bind(food.food_id);
        }
        query.push(") AND p.key IN (");
        let mut keys = query.separated(", ");
        for key in KEY_VALUES
            .iter()
            .chain([&parameters::DENSITY, &parameters::WASTE])
        {
            keys.push_bind(*key);
        }
        query.push(")");
        for row in query.build().fetch_all(&self.pool).await? {
            values
                .entry(row.get("food_id"))
                .or_default()
                .insert(row.get("key"), row.get("value"));
        }
        Ok(values)
    }

    /// One Food with every parameter.
    pub async fn food(&self, food_id: i64) -> Result<Food, FoodError> {
        let ready = self.ready()?;
        let row = sqlx::query(
            "SELECT food_id, name_da, name_en, food_group_id, food_group_da, food_group_en \
             FROM food WHERE food_id = ?",
        )
        .bind(food_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(FoodError::NotFound(food_id))?;
        let identity = identity(&row);
        let parameters: Vec<ParameterValue> = sqlx::query(
            "SELECT p.parameter_id, p.key, p.name_da, p.name_en, p.group_da, p.group_en, p.unit, \
                 p.per_100g, v.value \
             FROM parameter p \
             LEFT JOIN food_value v ON v.parameter_id = p.parameter_id AND v.food_id = ? \
             ORDER BY p.sort_key, p.parameter_id",
        )
        .bind(food_id)
        .fetch_all(&self.pool)
        .await?
        .iter()
        .map(|row| ParameterValue {
            key: row.get("key"),
            parameter_id: row.get("parameter_id"),
            name_da: row.get("name_da"),
            name_en: row.get("name_en"),
            group: ParameterGroup {
                name_da: row.get("group_da"),
                name_en: row.get("group_en"),
            },
            unit: row.get("unit"),
            per_100g: row.get("per_100g"),
            value: row.get("value"),
        })
        .collect();
        let value_of = |key: &'static str| {
            amount(
                key,
                parameters
                    .iter()
                    .find(|parameter| parameter.key == key)
                    .and_then(|parameter| parameter.value),
            )
        };
        Ok(Food {
            food_id: identity.food_id,
            name_da: identity.name_da,
            name_en: identity.name_en,
            food_group: identity.food_group,
            density: value_of(parameters::DENSITY),
            waste: value_of(parameters::WASTE),
            parameters,
            source: source(&ready.version),
        })
    }
}

impl FoodDatabase {
    /// The known per-100 g values of each Food in `food_ids`, by `FoodID`. A missing Food is
    /// [`FoodError::NotFound`].
    pub async fn snapshots(
        &self,
        food_ids: &[i64],
    ) -> Result<HashMap<i64, FoodSnapshot>, FoodError> {
        let ready = self.ready()?;
        let mut ids = food_ids.to_vec();
        ids.sort_unstable();
        ids.dedup();
        let mut snapshots = HashMap::new();
        if ids.is_empty() {
            return Ok(snapshots);
        }
        let mut query =
            QueryBuilder::new("SELECT food_id, name_da, name_en FROM food WHERE food_id IN (");
        let mut separated = query.separated(", ");
        for id in &ids {
            separated.push_bind(*id);
        }
        query.push(")");
        for row in query.build().fetch_all(&self.pool).await? {
            let food_id: i64 = row.get("food_id");
            snapshots.insert(
                food_id,
                FoodSnapshot {
                    food_id,
                    name_da: row.get("name_da"),
                    name_en: row.get("name_en"),
                    dataset_version: ready.version.clone(),
                    per_100g: Vec::new(),
                },
            );
        }
        if let Some(missing) = ids.iter().find(|id| !snapshots.contains_key(id)) {
            return Err(FoodError::NotFound(*missing));
        }
        let mut query = QueryBuilder::new(
            "SELECT v.food_id, p.key, v.value, p.unit FROM food_value v \
             JOIN parameter p ON p.parameter_id = v.parameter_id \
             WHERE p.per_100g = 1 AND p.unit IS NOT NULL AND v.food_id IN (",
        );
        let mut separated = query.separated(", ");
        for id in &ids {
            separated.push_bind(*id);
        }
        query.push(") ORDER BY p.sort_key, p.parameter_id");
        for row in query.build().fetch_all(&self.pool).await? {
            if let Some(snapshot) = snapshots.get_mut(&row.get::<i64, _>("food_id")) {
                snapshot.per_100g.push(SnapshotValue {
                    key: row.get("key"),
                    value: row.get("value"),
                    unit: row.get("unit"),
                });
            }
        }
        Ok(snapshots)
    }

    /// Every per-100 g parameter of the imported dataset in database order, with the dataset to
    /// credit.
    pub async fn nutrient_parameters(&self) -> Result<(Vec<NutrientParameter>, Source), FoodError> {
        let ready = self.ready()?;
        let parameters = sqlx::query(
            "SELECT key, name_da, name_en, group_da, group_en, unit FROM parameter \
             WHERE per_100g = 1 ORDER BY sort_key, parameter_id",
        )
        .fetch_all(&self.pool)
        .await?
        .iter()
        .map(|row| NutrientParameter {
            key: row.get("key"),
            name_da: row.get("name_da"),
            name_en: row.get("name_en"),
            group: ParameterGroup {
                name_da: row.get("group_da"),
                name_en: row.get("group_en"),
            },
            unit: row.get("unit"),
        })
        .collect();
        Ok((parameters, source(&ready.version)))
    }
}

impl FoodImport {
    /// Downloads and imports the pinned dataset. On failure the error is logged and any previous
    /// complete import stays in use; without one, lookups report the import failed.
    pub async fn run(self) {
        let Self {
            database,
            pin,
            cache_dir,
        } = self;
        let result = import::import(&database.pool, &pin, &cache_dir).await;
        if let Err(error) = &result {
            tracing::error!(%error, version = %pin.version, "The food database import failed");
        }
        // Read back what `food.db` holds now: the new import, or the previous one after a
        // rolled-back failure.
        let state = match load(&database.pool).await {
            Ok(Some(ready)) => State::Ready(ready),
            Ok(None) => State::Failed,
            Err(error) => {
                tracing::error!(%error, "The food database could not be read after import");
                State::Failed
            }
        };
        database.set_state(state);
    }
}

/// The complete import `food.db` holds, if any, with its search index.
async fn load(pool: &SqlitePool) -> Result<Option<Arc<Ready>>, sqlx::Error> {
    let Some(version) = sqlx::query_scalar::<_, String>("SELECT version FROM dataset WHERE id = 1")
        .fetch_optional(pool)
        .await?
    else {
        return Ok(None);
    };
    let foods = sqlx::query(
        "SELECT food_id, name_da, name_en, food_group_id, food_group_da, food_group_en \
         FROM food ORDER BY food_id",
    )
    .fetch_all(pool)
    .await?
    .iter()
    .map(identity)
    .collect();
    Ok(Some(Arc::new(Ready {
        version,
        index: SearchIndex::new(foods),
    })))
}

fn identity(row: &sqlx::sqlite::SqliteRow) -> FoodIdentity {
    let group = FoodGroup {
        id: row.get("food_group_id"),
        name_da: row.get("food_group_da"),
        name_en: row.get("food_group_en"),
    };
    let has_group = group.id.is_some() || group.name_da.is_some() || group.name_en.is_some();
    FoodIdentity {
        food_id: row.get("food_id"),
        name_da: row.get("name_da"),
        name_en: row.get("name_en"),
        food_group: has_group.then_some(group),
    }
}

/// A value in its vocabulary unit.
fn amount(key: &'static str, value: Option<f64>) -> Amount {
    Amount {
        value,
        unit: parameters::by_key(key).and_then(|parameter| Unit::symbol(parameter.unit)),
    }
}

fn source(version: &str) -> Source {
    Source {
        name: SOURCE_NAME,
        version: version.to_owned(),
        credit: format!("© {SOURCE_NAME}, version {version}"),
        licence: SOURCE_LICENCE,
    }
}
