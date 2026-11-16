//! Meals (`CONTEXT.md`, Meal): named, reusable lists of Portions the user logs again by name,
//! without analysing a photo.
//!
//! A Meal holds references, never values: each Portion is grams of a Food (by `FoodID`) or of a
//! linked Product (by barcode), and each Supplement Dose a number of doses of a Supplement, in the
//! order the user gave. A Meal holds at least one of either, so a morning supplement stack is a
//! Meal too. Logging a Meal snapshots its items
//! afresh (ADR 0012), so a re-linked Product or a corrected Food reaches the next log, and
//! editing or deleting a Meal never changes a Meal Log made from it.
//!
//! Names are unique case-insensitively, including non-ASCII letters. Saving follows the
//! `request_id` receipt pattern: a retry replays the accepted Meal, the same `request_id` with
//! changed arguments is a conflict, and a deleted Meal keeps its receipt so an old retry cannot
//! recreate it.
//!
//! Every item is checked when it is saved: a Food must be in the current Food database and a
//! barcode must be a linked Product. A Food keeps the names and dataset version it was saved
//! with; when a later dataset no longer has its `FoodID`, or gives that `FoodID` to another Food,
//! the Meal must be updated before it can be logged again ([`MealError::NeedsUpdate`]).

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
    /// The Meal names a Food the current Food database no longer has.
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

/// One item of a Meal: edible grams of a Food (`food_id`) or of a linked Product (`barcode`).
/// The same shape addresses an item, by its `food_id` or `barcode`, in a log's gram overrides.
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

/// Omitted fields keep their value. `portions` replaces every Food and Product item, and
/// `supplement_doses` every dose.
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

/// One item of a Meal as it is shown: its reference and grams, with names to recognise it by.
#[derive(Debug, Serialize, Deserialize)]
pub struct MealPortion {
    /// The Food; null for a Product.
    pub food_id: Option<i64>,
    /// The Product's barcode; null for a Food.
    pub barcode: Option<String>,
    pub grams: f64,
    /// The Food as it was named when saved, or the Food the Product is linked to now.
    pub name_da: String,
    pub name_en: String,
    /// The Product as its label names it; null for a Food.
    pub product: Option<PortionProduct>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Meal {
    pub id: String,
    pub name: String,
    /// In the order they were given.
    pub portions: Vec<MealPortion>,
    /// In the order they were given, each Supplement named as it is now.
    pub supplement_doses: Vec<DoseSummary>,
}

#[derive(Debug, Serialize)]
pub struct Saved {
    pub meal: Meal,
    pub replayed: bool,
}

#[derive(Debug, Serialize)]
pub struct Meals {
    /// By name.
    pub meals: Vec<Meal>,
}

/// A Meal to log: its name as saved, its items with the grams to log and its Supplements with
/// the doses to log.
pub(super) struct Resolved {
    pub name: String,
    pub items: Vec<(Item, f64)>,
    pub doses: Vec<(String, f64)>,
}

/// A Meal or Supplement name trimmed with inner whitespace collapsed, and the lowercased key
/// names are unique by.
pub(super) struct UniqueName {
    pub name: String,
    pub key: String,
}

impl UniqueName {
    /// Parses the name of a `noun` ("Meal", "Supplement") given in `field`.
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
