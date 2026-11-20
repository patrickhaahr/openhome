use crate::services::patch::Patch;
use serde::{Deserialize, Serialize};

mod doses;
mod label;
mod store;
pub use doses::{DoseRequest, DoseSummary, MAX_DOSES, MAX_SUPPLEMENT_DOSES, SupplementDose};
pub(super) use doses::{
    StoredDose, check_doses, dose_totals, insert_log_doses, insert_meal_doses, log_doses,
    log_doses_on, meal_doses, normalized, parse_doses, resolve,
};
pub(super) use label::convert_mass;
use label::{amounts, check_label, compounds};
pub(super) use store::stored;
pub use store::{add_supplement, delete_supplement, supplements, update_supplement};

pub const MAX_LABEL_ITEMS: usize = 100;
const MAX_GRAMS_PER_DOSE: f64 = 100.0;
const MAX_KCAL_PER_DOSE: f64 = 1_000.0;
const KJ_PER_KCAL: f64 = 4.184;

pub const FORMS: &[(&str, &[&str])] = &[
    ("vitamin_a", &["retinol", "beta_carotene"]),
    ("folate", &["folic_acid", "methylfolate"]),
    ("niacin_equivalent", &["nicotinic_acid", "nicotinamide"]),
    ("niacin", &["nicotinic_acid", "nicotinamide"]),
];

fn forms(key: &str) -> Option<&'static [&'static str]> {
    FORMS
        .iter()
        .find(|(parameter, _)| *parameter == key)
        .map(|(_, forms)| *forms)
}

#[derive(Debug, thiserror::Error)]
pub enum SupplementError {
    #[error("{0}")]
    Validation(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Conflict(String),
    #[error("Supplement database operation failed")]
    Database(#[from] sqlx::Error),
}

type Result<T> = std::result::Result<T, SupplementError>;

fn validation(message: String) -> SupplementError {
    SupplementError::Validation(message)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(rename_all = "snake_case")]
pub enum DoseUnit {
    Tablet,
    Capsule,
    Scoop,
    Ml,
}

impl DoseUnit {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Tablet => "tablet",
            Self::Capsule => "capsule",
            Self::Scoop => "scoop",
            Self::Ml => "ml",
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AmountRequest {
    pub key: String,
    pub value: f64,
    pub unit: String,
    #[serde(default)]
    pub form: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompoundRequest {
    pub name: String,
    pub value: f64,
    pub unit: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AddSupplement {
    pub name: String,
    pub dose_unit: DoseUnit,
    #[serde(default)]
    pub amounts: Vec<AmountRequest>,
    #[serde(default)]
    pub compounds: Vec<CompoundRequest>,
    pub request_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EditSupplement {
    #[serde(default)]
    pub name: Patch<String>,
    #[serde(default)]
    pub dose_unit: Patch<DoseUnit>,
    #[serde(default)]
    pub amounts: Patch<Vec<AmountRequest>>,
    #[serde(default)]
    pub compounds: Patch<Vec<CompoundRequest>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Amount {
    pub key: String,
    pub form: Option<String>,
    pub value: f64,
    pub unit: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Compound {
    pub name: String,
    pub value: f64,
    pub unit: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Supplement {
    pub id: String,
    pub name: String,
    pub dose_unit: DoseUnit,
    pub amounts: Vec<Amount>,
    pub compounds: Vec<Compound>,
}

#[derive(Debug, Serialize)]
pub struct Added {
    pub supplement: Supplement,
    pub replayed: bool,
}

#[derive(Debug, Serialize)]
pub struct Supplements {
    pub supplements: Vec<Supplement>,
}
