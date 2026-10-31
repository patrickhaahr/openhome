//! Nutrition Day: one Europe/Copenhagen calendar day's Meal Logs and the totals of every
//! nutrient eaten and taken, summed from the Portions' and Supplement Doses' snapshots and split
//! into food and supplement shares, with the compounds and Supplement Doses taken and the NOVA 1
//! share.

use std::collections::BTreeMap;

use serde::Serialize;
use sqlx::SqlitePool;

use super::food_database::{FoodDatabase, ParameterGroup, Source};
use super::intake::{Compounds, FormAmount, Intake};
use super::meal_logs::{
    self, KeyNutrients, MealLog, MealLogError, Selection, StoredLog, StoredPortion, Total,
    key_totals,
};
use super::nova::{NovaEnergy, NovaShare};
use super::products::{LABEL_SOURCE, LabelSource};
use super::supplements::{self, Compound, DoseSummary, StoredDose};
use crate::services::training::calendar::{self, CalendarDay};

/// The total of one nutrient over the day's Portions and Supplement Doses.
#[derive(Debug, Serialize)]
pub struct NutrientTotal {
    pub key: String,
    /// Null for a value whose parameter the current dataset lacks: one past Portions or a
    /// Supplement's label carry.
    pub name_da: Option<String>,
    pub name_en: Option<String>,
    pub group: Option<ParameterGroup>,
    pub unit: Option<String>,
    #[serde(flatten)]
    pub total: Total,
    /// The supplement share by chemical form, for parameters whose Upper Limits differ by form;
    /// left out when no dose names a form.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub supplement_forms: Vec<FormAmount>,
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
    /// Compounds without a database parameter, with the amounts taken, in the order first taken.
    pub compounds: Vec<Compound>,
    /// Each Supplement's doses added together, in the order first taken.
    pub supplement_doses: Vec<DoseSummary>,
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
    let doses: Vec<&StoredDose> = logs.iter().flat_map(|log| &log.doses).collect();
    let nutrient = |key: &str| {
        let intake = Intake::of(key, &portions, &doses);
        (intake.forms(), intake)
    };

    let mut nutrients: Vec<NutrientTotal> = parameters
        .into_iter()
        .map(|parameter| {
            let (supplement_forms, intake) = nutrient(&parameter.key);
            NutrientTotal {
                total: intake.into_total(parameter.unit.clone()),
                supplement_forms,
                key: parameter.key,
                name_da: Some(parameter.name_da),
                name_en: Some(parameter.name_en),
                group: Some(parameter.group),
                unit: parameter.unit,
            }
        })
        .collect();
    // Keep values whose parameter the current dataset does not define, by key: from past
    // Portions of a retired parameter, or from Supplement labels.
    let mut unlisted: BTreeMap<&str, &str> = BTreeMap::new();
    let portion_values = portions.iter().flat_map(|portion| {
        portion
            .values
            .iter()
            .map(|(key, stored)| (key.as_str(), stored.unit.as_str()))
    });
    let dose_values = doses.iter().flat_map(|dose| {
        dose.amounts
            .iter()
            .map(|amount| (amount.key.as_str(), amount.unit.as_str()))
    });
    for (key, unit) in portion_values.chain(dose_values) {
        if !nutrients.iter().any(|nutrient| nutrient.key == key) {
            unlisted.entry(key).or_insert(unit);
        }
    }
    for (key, unit) in unlisted {
        let (supplement_forms, intake) = nutrient(key);
        nutrients.push(NutrientTotal {
            total: intake.into_total(Some(unit.to_owned())),
            supplement_forms,
            key: key.to_owned(),
            name_da: None,
            name_en: None,
            group: None,
            unit: Some(unit.to_owned()),
        });
    }

    let totals = key_totals(&portions, &doses);
    Ok(NutritionDay {
        day: day.to_string(),
        time_zone: calendar::TIME_ZONE_NAME,
        day_start: calendar::iso(day.start()),
        day_end: calendar::iso(day.end()),
        totals,
        nutrients,
        compounds: Compounds::of(&doses).into_amounts(),
        supplement_doses: supplements::dose_totals(&doses),
        nova: NovaEnergy::of(&portions).summary(),
        meal_logs: logs
            .into_iter()
            .map(StoredLog::into_meal_log)
            .collect::<Result<_, _>>()?,
        source,
        label_source: LABEL_SOURCE,
    })
}
