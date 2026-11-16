use super::ValueSource;
use crate::services::nutrition::nova::NovaClass;
use crate::services::nutrition::parameters;
use crate::services::nutrition::supplements::SupplementDose;
use serde::{Deserialize, Serialize};

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
#[derive(Debug, Serialize, Deserialize)]
pub struct KeyNutrients<T> {
    pub energy_kcal: T,
    pub protein: T,
    pub available_carbohydrate: T,
    pub fat: T,
    pub dietary_fibre: T,
    pub alcohol: T,
}

impl<T> KeyNutrients<T> {
    pub(in crate::services::nutrition) fn from_fn(
        mut value: impl FnMut(&'static str) -> T,
    ) -> Self {
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
#[derive(Debug, Serialize, Deserialize)]
pub struct PortionAmount {
    pub value: Option<f64>,
    pub unit: Option<String>,
    pub source: Option<ValueSource>,
}

/// A sum over Portions and Supplement Doses. `food` adds the Portions that have a value and
/// `unknown_portions` counts those that do not; `supplements` adds the doses whose label lists
/// the value; `value` adds both. Each is null when nothing has a value, including when nothing
/// was eaten.
#[derive(Debug, Serialize, Deserialize)]
pub struct Total {
    pub value: Option<f64>,
    pub unit: Option<String>,
    pub unknown_portions: usize,
    pub food: Option<f64>,
    pub supplements: Option<f64>,
}

/// The Product a Portion was logged as, as it was named then.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PortionProduct {
    pub barcode: String,
    pub name: Option<String>,
    pub brand: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
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
    /// The Product's Open Food Facts class or the Food's class, as logged; null while
    /// unclassified.
    pub nova_class: Option<NovaClass>,
    pub nutrients: KeyNutrients<PortionAmount>,
}

/// Where the photo media routes serve stored photos, by photo ID.
pub const PHOTO_PATH: &str = "/api/nutrition/meal-photos";

#[derive(Debug, Serialize, Deserialize)]
pub struct PhotoThumbnail {
    pub width: u32,
    pub height: u32,
    pub url: String,
}

/// A Meal Log's AVIF photo; a new photo gets a new `id`, so its URLs never change content.
#[derive(Debug, Serialize, Deserialize)]
pub struct Photo {
    pub id: String,
    pub width: u32,
    pub height: u32,
    pub url: String,
    pub thumbnail: PhotoThumbnail,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MealLog {
    pub id: String,
    /// When it was eaten, in Copenhagen time with its offset.
    pub eaten_at: String,
    /// The Europe/Copenhagen calendar day it counts toward.
    pub day: String,
    /// The name of the Meal it was logged from, as named then; null otherwise.
    pub meal_name: Option<String>,
    pub note: Option<String>,
    pub photo: Option<Photo>,
    pub portions: Vec<Portion>,
    pub supplement_doses: Vec<SupplementDose>,
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
