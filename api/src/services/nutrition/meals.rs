use crate::services::nutrition::food_database::FoodError;
use crate::services::nutrition::meal_logs::{Item, PortionProduct};
use crate::services::nutrition::supplements::{DoseRequest, DoseSummary, SupplementError};
use crate::services::patch::Patch;
use serde::{Deserialize, Serialize};

mod read;
mod resolve;
mod validate;
mod write;
pub use read::meals;
use read::{find, load_on, not_found, stored_items_on};
pub(super) use resolve::resolve;
use validate::{CheckedItem, check, check_doses, check_items, doses, items, normalized};
pub use write::{delete_meal, save_meal, update_meal};

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
    #[error("{0}")]
    NeedsUpdate(String),
    #[error(transparent)]
    Food(#[from] FoodError),
    #[error(transparent)]
    Supplement(#[from] SupplementError),
    #[error("Meal database operation failed")]
    Database(#[from] sqlx::Error),
}

type Result<T> = std::result::Result<T, MealError>;

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
    #[serde(default)]
    pub portions: Vec<MealItemRequest>,
    #[serde(default)]
    pub supplement_doses: Vec<DoseRequest>,
    pub request_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EditMeal {
    #[serde(default)]
    pub name: Patch<String>,
    #[serde(default)]
    pub portions: Patch<Vec<MealItemRequest>>,
    #[serde(default)]
    pub supplement_doses: Patch<Vec<DoseRequest>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MealPortion {
    pub food_id: Option<i64>,
    pub barcode: Option<String>,
    pub grams: f64,
    pub name_da: String,
    pub name_en: String,
    pub product: Option<PortionProduct>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Meal {
    pub id: String,
    pub name: String,
    pub portions: Vec<MealPortion>,
    pub supplement_doses: Vec<DoseSummary>,
}

#[derive(Debug, Serialize)]
pub struct Saved {
    pub meal: Meal,
    pub replayed: bool,
}

#[derive(Debug, Serialize)]
pub struct Meals {
    pub meals: Vec<Meal>,
}

pub(super) struct Resolved {
    pub name: String,
    pub items: Vec<(Item, f64)>,
    pub doses: Vec<(String, f64)>,
}

pub(super) struct UniqueName {
    pub name: String,
    pub key: String,
}

impl UniqueName {
    pub(super) fn parse(text: &str, field: &str, noun: &str) -> std::result::Result<Self, String> {
        let name = text.split_whitespace().collect::<Vec<_>>().join(" ");
        if name.is_empty()
            || name.chars().count() > MAX_NAME_CHARS
            || name.chars().any(char::is_control)
        {
            return Err(format!(
                "{field} must be a {noun} name of 1-{MAX_NAME_CHARS} characters without control \
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
