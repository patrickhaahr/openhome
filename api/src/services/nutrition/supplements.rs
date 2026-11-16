//! Supplements (`CONTEXT.md`, Supplement): labelled products counted per dose (tablet, capsule,
//! scoop or ml) rather than per 100 g.
//!
//! A Supplement's label amounts are kept per dose. An amount of something the Food database has a
//! parameter for (vitamin D, zinc, EPA, energy, protein) is keyed and united as in the parameter
//! vocabulary, so a dose adds to the same totals as Portions and every total can be split into a
//! food share and a supplement share. Anything else (creatine) is a named compound with a mass,
//! reported as an amount taken.
//!
//! Some EFSA Upper Limits apply only to one chemical form, which supplements are the main source
//! of: folic acid rather than dietary folate, preformed vitamin A (retinol) rather than
//! carotenoids, and nicotinic acid or nicotinamide rather than niacin from protein. Amounts of
//! those parameters name their [`FORMS`], so the supplement share of each form can be checked.
//!
//! Adding follows the `request_id` receipt pattern, and names are unique case-insensitively like
//! Meal names. A Meal Log copies a dose's amounts when it is logged (ADR 0012), so editing or
//! deleting a Supplement never changes a past day; a Supplement a Meal names cannot be deleted.

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

/// The most amounts and compounds one Supplement lists together.
pub const MAX_LABEL_ITEMS: usize = 100;
/// The most of any mass one dose may hold.
const MAX_GRAMS_PER_DOSE: f64 = 100.0;
/// The most energy one dose may hold, e.g. a scoop of mass gainer.
const MAX_KCAL_PER_DOSE: f64 = 1_000.0;
const KJ_PER_KCAL: f64 = 4.184;

/// The parameters whose Upper Limits depend on the chemical form, with the forms an amount of
/// each must name.
pub const FORMS: &[(&str, &[&str])] = &[
    // Preformed vitamin A (retinol, retinyl acetate or palmitate) or provitamin A carotenoids.
    ("vitamin_a", &["retinol", "beta_carotene"]),
    // Synthetic folic acid or L-methylfolate (5-MTHF) salts.
    ("folate", &["folic_acid", "methylfolate"]),
    ("niacin_equivalent", &["nicotinic_acid", "nicotinamide"]),
    ("niacin", &["nicotinic_acid", "nicotinamide"]),
];

/// The forms an amount of `key` must name, if its Upper Limit depends on the form.
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

/// What one dose is: the label's per-dose unit.
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

/// One dose's amount of a Food database parameter, as the label states it.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AmountRequest {
    /// A per-100 g parameter key of the vocabulary, e.g. `vitamin_d`, `zinc` or `epa`.
    pub key: String,
    pub value: f64,
    /// The parameter's unit, or for a mass-based parameter any of g, mg and µg.
    pub unit: String,
    /// The chemical form, required for the parameters in [`FORMS`] and refused for others.
    #[serde(default)]
    pub form: Option<String>,
}

/// One dose's amount of a compound without a database parameter, e.g. creatine.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompoundRequest {
    pub name: String,
    pub value: f64,
    /// g, mg or µg.
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

/// Omitted fields keep their value; `amounts` and `compounds` each replace the whole list.
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

/// An amount of a parameter, in the parameter's unit.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Amount {
    pub key: String,
    /// One of the parameter's [`FORMS`]; null for parameters without forms.
    pub form: Option<String>,
    pub value: f64,
    pub unit: String,
}

/// An amount of a named compound.
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
    /// Per dose, in the order given.
    pub amounts: Vec<Amount>,
    /// Per dose, in the order given.
    pub compounds: Vec<Compound>,
}

#[derive(Debug, Serialize)]
pub struct Added {
    pub supplement: Supplement,
    pub replayed: bool,
}

#[derive(Debug, Serialize)]
pub struct Supplements {
    /// By name.
    pub supplements: Vec<Supplement>,
}
