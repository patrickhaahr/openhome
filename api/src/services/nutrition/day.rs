//! Nutrition Day: one Europe/Copenhagen calendar day's Meal Logs and the totals of every
//! nutrient eaten, summed from the Portions' snapshots, with the NOVA 1 share.

use std::collections::BTreeMap;

use serde::Serialize;
use sqlx::SqlitePool;

use super::food_database::{FoodDatabase, ParameterGroup, Source};
use super::meal_logs::{
    self, KeyNutrients, MealLog, MealLogError, Selection, StoredLog, StoredPortion, Total,
    key_totals,
};
use super::nova::{NovaEnergy, NovaShare};
use super::products::{LABEL_SOURCE, LabelSource};
use crate::services::training::calendar::{self, CalendarDay};

/// The total of one nutrient over the day's Portions.
#[derive(Debug, Serialize)]
pub struct NutrientTotal {
    pub key: String,
    /// Null for a value only past Portions carry, whose parameter the current dataset lacks.
    pub name_da: Option<String>,
    pub name_en: Option<String>,
    pub group: Option<ParameterGroup>,
    pub unit: Option<String>,
    #[serde(flatten)]
    pub total: Total,
}

#[derive(Debug, Serialize)]
pub struct NutritionDay {
    pub day: String,
    pub time_zone: &'static str,
    /// Local midnight at the start of the day and the next one; a day can last 23 or 25 hours.
    pub day_start: String,
    pub day_end: String,
    /// Energy and macronutrients.
    pub totals: KeyNutrients<Total>,
    /// Every per-100 g parameter in database order.
    pub nutrients: Vec<NutrientTotal>,
    /// The share of classified energy from NOVA Class 1 Portions.
    pub nova: NovaShare,
    /// In time order.
    pub meal_logs: Vec<MealLog>,
    pub source: Source,
    /// Open Food Facts, to credit beside Product Portions' label values.
    pub label_source: LabelSource,
}

/// The day's Meal Logs and the totals of every parameter, with units; unknown is null.
pub async fn nutrition_day(
    db: &SqlitePool,
    foods: &FoodDatabase,
    day: CalendarDay,
) -> Result<NutritionDay, MealLogError> {
    let (parameters, source) = foods.nutrient_parameters().await?;
    let logs = meal_logs::load(db, &Selection::Days(day, day), true).await?;
    let portions: Vec<&StoredPortion> = logs.iter().flat_map(|log| &log.portions).collect();

    let mut nutrients: Vec<NutrientTotal> = parameters
        .into_iter()
        .map(|parameter| NutrientTotal {
            total: meal_logs::total(&parameter.key, parameter.unit.clone(), &portions),
            key: parameter.key,
            name_da: Some(parameter.name_da),
            name_en: Some(parameter.name_en),
            group: Some(parameter.group),
            unit: parameter.unit,
        })
        .collect();
    // Keep values whose parameter the current dataset no longer defines, by key.
    let mut retired: BTreeMap<&str, &str> = BTreeMap::new();
    for portion in &portions {
        for (key, stored) in &portion.values {
            if !nutrients.iter().any(|nutrient| &nutrient.key == key) {
                retired.insert(key, &stored.unit);
            }
        }
    }
    for (key, unit) in retired {
        nutrients.push(NutrientTotal {
            total: meal_logs::total(key, Some(unit.to_owned()), &portions),
            key: key.to_owned(),
            name_da: None,
            name_en: None,
            group: None,
            unit: Some(unit.to_owned()),
        });
    }

    let totals = key_totals(&portions);
    Ok(NutritionDay {
        day: day.to_string(),
        time_zone: calendar::TIME_ZONE_NAME,
        day_start: calendar::iso(day.start()),
        day_end: calendar::iso(day.end()),
        totals,
        nutrients,
        nova: NovaEnergy::of(&portions).summary(),
        meal_logs: logs
            .into_iter()
            .map(StoredLog::into_meal_log)
            .collect::<Result<_, _>>()?,
        source,
        label_source: LABEL_SOURCE,
    })
}
