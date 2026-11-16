//! Meal Logs (`CONTEXT.md`, Nutrition): what the user ate, at an explicit time.
//!
//! A Meal Log belongs to the Europe/Copenhagen calendar day of its time, so a backfilled meal
//! lands on the day it was eaten. Each Portion copies every known per-100 g value when it is
//! logged, tagged with where the value came from (ADR 0012): a Product Portion takes its label
//! values first and its linked Food's values for the rest, and the agent may estimate values no
//! source has. A later Food database import or re-link never changes a past Meal Log, and
//! replacing a Portion is the only way to correct one.
//!
//! Logging follows the `request_id` receipt pattern: a retry replays the original result, the
//! same `request_id` with changed arguments is a conflict, and a deleted Meal Log keeps its
//! receipt so an old retry cannot recreate it.
//!
//! Supplement Doses sit beside the Portions and copy their Supplement's per-dose amounts the same
//! way, so editing or deleting a Supplement never changes a past Meal Log. A Meal Log holds at
//! least one Portion or Supplement Dose.
//!
//! An optional photo is encoded before anything is written and stored before the Meal Log row
//! that references it, so a rejected photo writes nothing and a failed write removes its files.
//! A replaced, removed or deleted photo's files are removed only after the row change commits.
//! `meal_photos` describes the ownership stages and the cancellation and crash windows.

use crate::services::nutrition::food_database::FoodError;
use crate::services::nutrition::meal_photos::PhotoError;
use crate::services::nutrition::meals::MealError;
use crate::services::nutrition::supplements::SupplementError;

mod contents;
mod edit;
mod log;
mod prepare;
mod read;
mod requests;
mod stored;
mod validation;
mod views;
use contents::{Contents, contents};
pub use edit::{delete_meal_log, replace_photo, update_meal_log};
pub use log::log_meal;
use log::{PhotoColumns, store_photo};
use prepare::{PreparedPortion, insert_portions, prepare, prepare_doses};
pub use read::meal_logs;
pub(super) use read::{Selection, Values, load, load_on};
use read::{find, not_found};
pub use requests::{
    EditMealLog, LogMeal, MealLogsQuery, OverrideRequest, PortionRequest, ValueSource,
};
pub(super) use stored::{StoredLog, StoredPortion, StoredValue, key_totals, round};
use stored::{StoredPhoto, vocabulary_unit};
pub use validation::parse_day;
pub(crate) use validation::parse_request_id;
pub(super) use validation::{Item, check_grams, parse_item};
use validation::{
    ValidPortion, check_items, doses, eaten_at, note, portions, request_id, required, utc_text,
};
pub use views::{
    KEY_NUTRIENTS, KeyNutrients, Logged, MealLog, MealLogs, PHOTO_PATH, Photo, PhotoThumbnail,
    Portion, PortionAmount, PortionProduct, Total,
};

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
    /// A Product's linked `FoodID` names another Food, or none, in the current Food database.
    #[error("{0}")]
    ProductNeedsRelink(String),
    #[error(transparent)]
    Food(#[from] FoodError),
    #[error(transparent)]
    Photo(#[from] PhotoError),
    #[error(transparent)]
    Meal(#[from] MealError),
    #[error(transparent)]
    Supplement(#[from] SupplementError),
    #[error("Meal Log database operation failed")]
    Database(#[from] sqlx::Error),
}

type Result<T> = std::result::Result<T, MealLogError>;
