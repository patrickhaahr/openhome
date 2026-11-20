use super::ValueSource;
use crate::services::nutrition::nova::NovaClass;
use crate::services::nutrition::parameters;
use crate::services::nutrition::supplements::SupplementDose;
use serde::{Deserialize, Serialize};

pub const KEY_NUTRIENTS: [&str; 6] = [
    parameters::ENERGY_KCAL,
    parameters::PROTEIN,
    parameters::AVAILABLE_CARBOHYDRATE,
    parameters::FAT,
    parameters::DIETARY_FIBRE,
    parameters::ALCOHOL,
];

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

#[derive(Debug, Serialize, Deserialize)]
pub struct PortionAmount {
    pub value: Option<f64>,
    pub unit: Option<String>,
    pub source: Option<ValueSource>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Total {
    pub value: Option<f64>,
    pub unit: Option<String>,
    pub unknown_portions: usize,
    pub food: Option<f64>,
    pub supplements: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PortionProduct {
    pub barcode: String,
    pub name: Option<String>,
    pub brand: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Portion {
    pub food_id: i64,
    pub product: Option<PortionProduct>,
    pub name_da: String,
    pub name_en: String,
    pub grams: f64,
    pub dataset_version: String,
    pub nova_class: Option<NovaClass>,
    pub nutrients: KeyNutrients<PortionAmount>,
}

pub const PHOTO_PATH: &str = "/api/nutrition/meal-photos";

#[derive(Debug, Serialize, Deserialize)]
pub struct PhotoThumbnail {
    pub width: u32,
    pub height: u32,
    pub url: String,
}

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
    pub eaten_at: String,
    pub day: String,
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
    pub meal_logs: Vec<MealLog>,
}
