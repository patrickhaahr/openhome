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
pub const MAX_RANGE_DAYS: i64 = 90;
const MAX_REQUEST_ID_CHARS: usize = 128;
const FUTURE_TOLERANCE_MINUTES: i64 = 5;

#[derive(Debug, thiserror::Error)]
pub enum MealLogError {
    #[error("{0}")]
    Validation(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Conflict(String),
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
