use super::{SOURCE_LICENCE, SOURCE_NAME};
use crate::services::nutrition::nova::NovaClass;
use crate::services::nutrition::parameters::{self, Unit};
use serde::Serialize;
use thiserror::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Unavailable {
    Importing,
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

#[derive(Debug, Serialize)]
pub struct Amount {
    pub value: Option<f64>,
    pub unit: Option<&'static str>,
}

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
    pub density: Amount,
    pub waste: Amount,
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
    pub nova_class: Option<NovaClass>,
    pub parameters: Vec<ParameterValue>,
    pub source: Source,
}

#[derive(Debug)]
pub struct FoodSnapshot {
    pub food_id: i64,
    pub name_da: String,
    pub name_en: String,
    pub dataset_version: String,
    pub per_100g: Vec<SnapshotValue>,
}

#[derive(Debug, Serialize)]
pub struct SnapshotValue {
    pub key: String,
    pub value: f64,
    pub unit: String,
}

#[derive(Debug)]
pub struct NutrientParameter {
    pub key: String,
    pub name_da: String,
    pub name_en: String,
    pub group: ParameterGroup,
    pub unit: Option<String>,
}

pub(super) fn amount(key: &'static str, value: Option<f64>) -> Amount {
    Amount {
        value,
        unit: parameters::by_key(key).and_then(|parameter| Unit::symbol(parameter.unit)),
    }
}

#[must_use]
pub(in crate::services::nutrition) fn source(version: &str) -> Source {
    Source {
        name: SOURCE_NAME,
        version: version.to_owned(),
        credit: format!("© {SOURCE_NAME}, version {version}"),
        licence: SOURCE_LICENCE,
    }
}
