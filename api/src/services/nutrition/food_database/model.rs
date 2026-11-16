use super::{SOURCE_LICENCE, SOURCE_NAME};
use crate::services::nutrition::nova::NovaClass;
use crate::services::nutrition::parameters::{self, Unit};
use serde::Serialize;
use thiserror::Error;

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
    /// The user's NOVA Class for the Food, kept in `app.db`; null while unclassified.
    pub nova_class: Option<NovaClass>,
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
    /// The user's NOVA Class for the Food, kept in `app.db`; null while unclassified.
    pub nova_class: Option<NovaClass>,
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

/// A value in its vocabulary unit.
pub(super) fn amount(key: &'static str, value: Option<f64>) -> Amount {
    Amount {
        value,
        unit: parameters::by_key(key).and_then(|parameter| Unit::symbol(parameter.unit)),
    }
}

/// The credit for values from `version` of the Danish Food Composition Database.
#[must_use]
pub(in crate::services::nutrition) fn source(version: &str) -> Source {
    Source {
        name: SOURCE_NAME,
        version: version.to_owned(),
        credit: format!("© {SOURCE_NAME}, version {version}"),
        licence: SOURCE_LICENCE,
    }
}
