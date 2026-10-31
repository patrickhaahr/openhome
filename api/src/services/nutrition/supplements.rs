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

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use uuid::Uuid;

use super::meal_logs::parse_request_id;
use super::meals::UniqueName;
use super::open_food_facts::significant;
use super::parameters::{self, Unit};
use crate::services::patch::Patch;

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
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Amount {
    pub key: String,
    /// One of the parameter's [`FORMS`]; null for parameters without forms.
    pub form: Option<String>,
    pub value: f64,
    pub unit: String,
}

/// An amount of a named compound.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Compound {
    pub name: String,
    pub value: f64,
    pub unit: String,
}

#[derive(Debug, Serialize)]
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

/// A mass unit's symbol and how many of it one gram is.
fn mass_unit(text: &str) -> Option<(&'static str, f64)> {
    match text {
        "g" => Some(("g", 1.0)),
        "mg" => Some(("mg", 1_000.0)),
        // The micro sign, the Greek mu and ASCII spellings.
        "µg" | "μg" | "ug" | "mcg" => Some(("µg", 1_000_000.0)),
        _ => None,
    }
}

fn amount(request: &AmountRequest) -> std::result::Result<Amount, String> {
    let key = request.key.trim();
    let parameter = parameters::by_key(key)
        .filter(|parameter| parameter.unit.per_100g())
        .ok_or_else(|| {
            format!(
                "amounts: {key} is not a Food database parameter key; give vitamins, minerals, \
                 fatty acids, energy and macronutrients by their key (vitamin_d, zinc, epa, \
                 energy_kcal, protein) and anything else in compounds"
            )
        })?;
    let symbol = parameter.unit.symbol().unwrap_or_default();
    let per_gram = parameter.unit.per_gram();
    let unit = request.unit.trim();
    let value = if unit == symbol {
        request.value
    } else {
        match (per_gram, mass_unit(unit)) {
            (Some(to), Some((_, from))) => request.value * to / from,
            (Some(_), None) => {
                return Err(format!("amounts.{key}: unit must be {symbol}, g, mg or µg"));
            }
            (None, _) => return Err(format!("amounts.{key}: unit must be {symbol}")),
        }
    };
    let limit = match parameter.unit {
        Unit::Kilocalorie => MAX_KCAL_PER_DOSE,
        Unit::Kilojoule => MAX_KCAL_PER_DOSE * KJ_PER_KCAL,
        _ => per_gram.map_or(f64::MAX, |per_gram| MAX_GRAMS_PER_DOSE * per_gram),
    };
    if !value.is_finite() || !(0.0..=limit).contains(&value) {
        return Err(format!(
            "amounts.{key} must be between 0 and {limit} {symbol} per dose"
        ));
    }
    let form = match (forms(key), request.form.as_deref().map(str::trim)) {
        (None, None) => None,
        (None, Some(_)) => {
            return Err(format!(
                "amounts.{key} has no forms to tell apart; leave form out"
            ));
        }
        (Some(forms), Some(form)) if forms.contains(&form) => Some(form.to_owned()),
        (Some(forms), _) => {
            return Err(format!(
                "amounts.{key} needs form, one of {}: Upper Limits differ by form, so read it \
                 from the label's ingredients (folic acid, retinyl acetate, nicotinamide)",
                forms.join(", ")
            ));
        }
    };
    Ok(Amount {
        key: parameter.key.to_owned(),
        form,
        value: significant(value),
        unit: symbol.to_owned(),
    })
}

/// The parameter key a compound name would be written as, e.g. `Vitamin D` as `vitamin_d`.
fn as_key(name: &str) -> String {
    name.split(|character: char| !character.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}

fn compound(request: &CompoundRequest) -> std::result::Result<Compound, String> {
    let name = UniqueName::parse(&request.name, "compounds.name", "compound")?;
    if let Some(parameter) = parameters::by_key(&as_key(&name.key)) {
        return Err(format!(
            "compounds: {} is the Food database parameter {}; give it in amounts",
            name.name, parameter.key
        ));
    }
    let Some((unit, per_gram)) = mass_unit(request.unit.trim()) else {
        return Err(format!("compounds.{}: unit must be g, mg or µg", name.name));
    };
    let limit = MAX_GRAMS_PER_DOSE * per_gram;
    if !request.value.is_finite() || !(0.0..=limit).contains(&request.value) {
        return Err(format!(
            "compounds.{} must be between 0 and {limit} {unit} per dose",
            name.name
        ));
    }
    Ok(Compound {
        name: name.name,
        value: significant(request.value),
        unit: unit.to_owned(),
    })
}

/// Completes energy in kcal and kJ from each other, as Product labels are.
fn complete_energy(mut amounts: Vec<Amount>) -> Vec<Amount> {
    let find = |amounts: &[Amount], key: &str| {
        amounts
            .iter()
            .find(|amount| amount.key == key)
            .map(|amount| amount.value)
    };
    let kcal = find(&amounts, parameters::ENERGY_KCAL);
    let kj = find(&amounts, parameters::ENERGY_KJ);
    let completed = match (kcal, kj) {
        (Some(kcal), None) => Some((parameters::ENERGY_KJ, "kJ", kcal * KJ_PER_KCAL)),
        (None, Some(kj)) => Some((parameters::ENERGY_KCAL, "kcal", kj / KJ_PER_KCAL)),
        _ => None,
    };
    if let Some((key, unit, value)) = completed {
        amounts.push(Amount {
            key: key.to_owned(),
            form: None,
            value: significant(value),
            unit: unit.to_owned(),
        });
    }
    amounts
}

fn amounts(requests: &[AmountRequest]) -> Result<Vec<Amount>> {
    let amounts = requests
        .iter()
        .map(amount)
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(validation)?;
    let mut seen = HashSet::new();
    for amount in &amounts {
        if !seen.insert((&amount.key, &amount.form)) {
            let form = amount
                .form
                .as_ref()
                .map(|form| format!(" as {form}"))
                .unwrap_or_default();
            return Err(validation(format!(
                "amounts list {}{form} twice; give each parameter once per dose",
                amount.key
            )));
        }
    }
    Ok(complete_energy(amounts))
}

fn compounds(requests: &[CompoundRequest]) -> Result<Vec<Compound>> {
    let compounds = requests
        .iter()
        .map(compound)
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(validation)?;
    let mut seen = HashSet::new();
    for compound in &compounds {
        if !seen.insert(compound.name.to_lowercase()) {
            return Err(validation(format!(
                "compounds list {} twice; give each compound once per dose",
                compound.name
            )));
        }
    }
    Ok(compounds)
}

/// A Supplement lists 1-[`MAX_LABEL_ITEMS`] amounts and compounds together.
fn check_label(amounts: &[Amount], compounds: &[Compound]) -> Result<()> {
    let items = amounts.len() + compounds.len();
    if items == 0 || items > MAX_LABEL_ITEMS {
        return Err(validation(format!(
            "a Supplement lists 1-{MAX_LABEL_ITEMS} amounts and compounds per dose together"
        )));
    }
    Ok(())
}

/// Every Supplement by name, or the one with `id`.
async fn load(db: &SqlitePool, id: Option<&str>) -> Result<Vec<Supplement>> {
    let rows = sqlx::query!(
        r#"SELECT id AS "id!", name, dose_unit AS "dose_unit: DoseUnit" FROM supplements
        WHERE ?1 IS NULL OR id = ?1 ORDER BY name_key"#,
        id
    )
    .fetch_all(db)
    .await?;
    let mut amounts: HashMap<String, Vec<Amount>> = HashMap::new();
    for row in sqlx::query!(
        "SELECT supplement_id, key, form, value, unit FROM supplement_amounts
        WHERE ?1 IS NULL OR supplement_id = ?1 ORDER BY supplement_id, position",
        id
    )
    .fetch_all(db)
    .await?
    {
        amounts.entry(row.supplement_id).or_default().push(Amount {
            key: row.key,
            form: row.form,
            value: row.value,
            unit: row.unit,
        });
    }
    let mut compounds: HashMap<String, Vec<Compound>> = HashMap::new();
    for row in sqlx::query!(
        "SELECT supplement_id, name, value, unit FROM supplement_compounds
        WHERE ?1 IS NULL OR supplement_id = ?1 ORDER BY supplement_id, position",
        id
    )
    .fetch_all(db)
    .await?
    {
        compounds
            .entry(row.supplement_id)
            .or_default()
            .push(Compound {
                name: row.name,
                value: row.value,
                unit: row.unit,
            });
    }
    Ok(rows
        .into_iter()
        .map(|row| Supplement {
            amounts: amounts.remove(&row.id).unwrap_or_default(),
            compounds: compounds.remove(&row.id).unwrap_or_default(),
            id: row.id,
            name: row.name,
            dose_unit: row.dose_unit,
        })
        .collect())
}

async fn find(db: &SqlitePool, id: &str) -> Result<Option<Supplement>> {
    Ok(load(db, Some(id)).await?.into_iter().next())
}

fn not_found() -> SupplementError {
    SupplementError::NotFound("Supplement not found; list supplements for its id".into())
}

/// The Supplements with these IDs, by ID; missing IDs are left out.
pub(super) async fn stored(db: &SqlitePool, ids: &[&str]) -> Result<HashMap<String, Supplement>> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    Ok(load(db, None)
        .await?
        .into_iter()
        .filter(|supplement| ids.contains(&supplement.id.as_str()))
        .map(|supplement| (supplement.id.clone(), supplement))
        .collect())
}

/// Every Supplement by name.
pub async fn supplements(db: &SqlitePool) -> Result<Supplements> {
    Ok(Supplements {
        supplements: load(db, None).await?,
    })
}

async fn replay(db: &SqlitePool, request_id: &str, request: &str) -> Result<Option<Added>> {
    let Some(receipt) = sqlx::query!(
        "SELECT supplement_id, request FROM supplement_receipts WHERE request_id = ?",
        request_id
    )
    .fetch_optional(db)
    .await?
    else {
        return Ok(None);
    };
    if receipt.request != request {
        return Err(SupplementError::Conflict(
            "request_id was already used for a different Supplement; use a new ID for a new \
             Supplement"
                .into(),
        ));
    }
    let supplement = find(db, &receipt.supplement_id).await?.ok_or_else(|| {
        SupplementError::Conflict(
            "The retried Supplement was deleted; use a new request_id to add it again".into(),
        )
    })?;
    Ok(Some(Added {
        supplement,
        replayed: true,
    }))
}

/// Maps the unique name index to a conflict.
fn unique_name(error: sqlx::Error, name: &str) -> SupplementError {
    match &error {
        sqlx::Error::Database(database) if database.is_unique_violation() => {
            SupplementError::Conflict(format!(
                "A Supplement named {name} already exists; choose another name, or \
                 update_supplement that Supplement"
            ))
        }
        _ => error.into(),
    }
}

/// Writes a Supplement's amounts and compounds; the caller removes the previous ones first.
async fn insert_label(
    transaction: &mut sqlx::SqliteConnection,
    id: &str,
    amounts: &[Amount],
    compounds: &[Compound],
) -> Result<()> {
    for (position, amount) in amounts.iter().enumerate() {
        let position = position as i64;
        sqlx::query!(
            "INSERT INTO supplement_amounts (supplement_id, position, key, form, value, unit) VALUES (?, ?, ?, ?, ?, ?)",
            id, position, amount.key, amount.form, amount.value, amount.unit)
            .execute(&mut *transaction)
            .await?;
    }
    for (position, compound) in compounds.iter().enumerate() {
        let position = position as i64;
        sqlx::query!(
            "INSERT INTO supplement_compounds (supplement_id, position, name, value, unit) VALUES (?, ?, ?, ?, ?)",
            id, position, compound.name, compound.value, compound.unit)
            .execute(&mut *transaction)
            .await?;
    }
    Ok(())
}

/// Adds a Supplement with its per-dose label, with an immutable receipt for its `request_id`.
/// Rejected requests consume no `request_id` and write nothing.
pub async fn add_supplement(db: &SqlitePool, request: AddSupplement) -> Result<Added> {
    let request_id = parse_request_id(&request.request_id).map_err(validation)?;
    let name = UniqueName::parse(&request.name, "name", "Supplement").map_err(validation)?;
    let amounts = amounts(&request.amounts)?;
    let compounds = compounds(&request.compounds)?;
    check_label(&amounts, &compounds)?;
    let normalized = serde_json::json!({
        "name": name.name,
        "dose_unit": request.dose_unit,
        "amounts": amounts,
        "compounds": compounds,
    })
    .to_string();
    if let Some(added) = replay(db, &request_id, &normalized).await? {
        return Ok(added);
    }

    let id = Uuid::new_v4().to_string();
    let mut transaction = db.begin().await?;
    // Claim first so simultaneous retries serialize on the receipt.
    let claimed = sqlx::query_scalar!(
        "INSERT INTO supplement_receipts (request_id, supplement_id, request) VALUES (?, ?, ?) ON CONFLICT (request_id) DO NOTHING RETURNING request_id",
        request_id, id, normalized)
        .fetch_optional(&mut *transaction)
        .await?;
    if claimed.is_none() {
        transaction.rollback().await?;
        return replay(db, &request_id, &normalized).await?.ok_or_else(|| {
            SupplementError::Conflict(
                "Retry receipt unavailable; list supplements before retrying".into(),
            )
        });
    }
    let dose_unit = request.dose_unit.as_str();
    sqlx::query!(
        "INSERT INTO supplements (id, name, name_key, dose_unit) VALUES (?, ?, ?, ?)",
        id,
        name.name,
        name.key,
        dose_unit
    )
    .execute(&mut *transaction)
    .await
    .map_err(|error| unique_name(error, &name.name))?;
    insert_label(&mut transaction, &id, &amounts, &compounds).await?;
    transaction.commit().await?;
    Ok(Added {
        supplement: find(db, &id).await?.ok_or_else(not_found)?,
        replayed: false,
    })
}

fn required<T>(patch: Patch<T>, field: &str) -> Result<Option<T>> {
    match patch {
        Patch::Missing => Ok(None),
        Patch::Value(Some(value)) => Ok(Some(value)),
        Patch::Value(None) => Err(validation(format!("{field} cannot be null"))),
    }
}

/// Renames a Supplement, changes its dose unit or replaces its amounts or compounds, together.
/// Meal Logs keep the doses they copied; Meals naming it log its new label from now on.
pub async fn update_supplement(
    db: &SqlitePool,
    id: &str,
    edit: EditSupplement,
) -> Result<Supplement> {
    let name = required(edit.name, "name")?
        .map(|name| UniqueName::parse(&name, "name", "Supplement").map_err(validation))
        .transpose()?;
    let dose_unit = required(edit.dose_unit, "dose_unit")?;
    let new_amounts = required(edit.amounts, "amounts")?
        .map(|requests| amounts(&requests))
        .transpose()?;
    let new_compounds = required(edit.compounds, "compounds")?
        .map(|requests| compounds(&requests))
        .transpose()?;
    let current = find(db, id).await?.ok_or_else(not_found)?;
    let amounts = new_amounts.unwrap_or(current.amounts);
    let compounds = new_compounds.unwrap_or(current.compounds);
    check_label(&amounts, &compounds)?;

    let mut transaction = db.begin().await?;
    let (display, key) = name
        .as_ref()
        .map(|name| (name.name.as_str(), name.key.as_str()))
        .unzip();
    let dose_unit = dose_unit.map(DoseUnit::as_str);
    let updated = sqlx::query!(
        r#"UPDATE supplements SET name = COALESCE(?1, name), name_key = COALESCE(?2, name_key),
        dose_unit = COALESCE(?3, dose_unit), updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE id = ?4"#,
        display,
        key,
        dose_unit,
        id
    )
    .execute(&mut *transaction)
    .await
    .map_err(|error| unique_name(error, display.unwrap_or_default()))?;
    if updated.rows_affected() == 0 {
        return Err(not_found());
    }
    sqlx::query!("DELETE FROM supplement_amounts WHERE supplement_id = ?", id)
        .execute(&mut *transaction)
        .await?;
    sqlx::query!(
        "DELETE FROM supplement_compounds WHERE supplement_id = ?",
        id
    )
    .execute(&mut *transaction)
    .await?;
    insert_label(&mut transaction, id, &amounts, &compounds).await?;
    transaction.commit().await?;
    find(db, id).await?.ok_or_else(not_found)
}

/// Deletes a Supplement no Meal names. Meal Logs keep the doses they copied, and its receipt
/// stays, so an old retry cannot recreate it.
pub async fn delete_supplement(db: &SqlitePool, id: &str) -> Result<()> {
    let meals = sqlx::query_scalar!(
        "SELECT DISTINCT m.name FROM meal_supplement_doses d JOIN meals m ON m.id = d.meal_id
        WHERE d.supplement_id = ? ORDER BY m.name_key",
        id
    )
    .fetch_all(db)
    .await?;
    if !meals.is_empty() {
        return Err(SupplementError::Conflict(format!(
            "The Meals {} take this Supplement; update_meal them without it before deleting it",
            meals.join(", ")
        )));
    }
    let deleted = sqlx::query!("DELETE FROM supplements WHERE id = ?", id)
        .execute(db)
        .await
        .map_err(|error| match &error {
            // A Meal saved with it since the check above.
            sqlx::Error::Database(database) if database.is_foreign_key_violation() => {
                SupplementError::Conflict(
                    "A Meal takes this Supplement; update_meal it without it before deleting it"
                        .into(),
                )
            }
            _ => error.into(),
        })?;
    if deleted.rows_affected() == 0 {
        return Err(not_found());
    }
    Ok(())
}

/// The most doses of one Supplement in one Meal Log or Meal, e.g. 100 ml.
pub const MAX_DOSES: f64 = 100.0;
/// The most Supplement Doses one Meal Log or Meal lists.
pub const MAX_SUPPLEMENT_DOSES: usize = 50;

/// One Supplement Dose: a number of doses of a Supplement, in a Meal Log or a Meal. In a
/// `log_meal` override it names the Meal's dose whose count changes.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DoseRequest {
    pub supplement_id: String,
    pub doses: f64,
}

/// Checks a dose count: above 0 and at most [`MAX_DOSES`]; halves and millilitres are fine.
pub(super) fn check_doses(doses: f64) -> std::result::Result<(), String> {
    if !doses.is_finite() || doses <= 0.0 || doses > MAX_DOSES {
        return Err(format!(
            "doses must be above 0 and at most {MAX_DOSES} of the Supplement's dose unit"
        ));
    }
    Ok(())
}

/// Parses up to [`MAX_SUPPLEMENT_DOSES`] doses, each Supplement listed once.
pub(super) fn parse_doses(
    requests: Vec<DoseRequest>,
) -> std::result::Result<Vec<(String, f64)>, String> {
    if requests.len() > MAX_SUPPLEMENT_DOSES {
        return Err(format!(
            "supplement_doses must list at most {MAX_SUPPLEMENT_DOSES} Supplement Doses"
        ));
    }
    let mut doses: Vec<(String, f64)> = Vec::with_capacity(requests.len());
    for request in requests {
        let id = request.supplement_id.trim().to_owned();
        check_doses(request.doses)?;
        if doses.iter().any(|(other, _)| *other == id) {
            return Err(format!(
                "supplement_id {id} is listed twice; list each Supplement once with its doses \
                 added together"
            ));
        }
        doses.push((id, request.doses));
    }
    Ok(doses)
}

/// The receipt form of a dose.
pub(super) fn normalized(supplement_id: &str, doses: f64) -> serde_json::Value {
    serde_json::json!({"supplement_id": supplement_id, "doses": doses})
}

fn unknown(id: &str) -> SupplementError {
    SupplementError::NotFound(format!(
        "Supplement {id} not found; list supplements for its id, or add_supplement it from its \
         label first"
    ))
}

/// The Supplements `doses` name, in order; any unknown Supplement is [`SupplementError::NotFound`].
pub(super) async fn resolve(
    db: &SqlitePool,
    doses: &[(String, f64)],
) -> Result<Vec<(Supplement, f64)>> {
    let ids: Vec<&str> = doses.iter().map(|(id, _)| id.as_str()).collect();
    let mut stored = stored(db, &ids).await?;
    doses
        .iter()
        .map(|(id, count)| {
            stored
                .remove(id)
                .map(|supplement| (supplement, *count))
                .ok_or_else(|| unknown(id))
        })
        .collect()
}

/// A dose as a Meal lists it, and a day's doses of one Supplement added together.
#[derive(Debug, Serialize)]
pub struct DoseSummary {
    pub supplement_id: String,
    /// The Supplement as it is named now in a Meal, or as it was logged in a day.
    pub name: String,
    pub dose_unit: DoseUnit,
    pub doses: f64,
}

/// Writes a Meal's doses, replacing none: the caller removes the previous doses first.
pub(super) async fn insert_meal_doses(
    transaction: &mut sqlx::SqliteConnection,
    meal_id: &str,
    doses: &[(String, f64)],
) -> Result<()> {
    for (position, (supplement_id, doses)) in doses.iter().enumerate() {
        let position = position as i64;
        sqlx::query!(
            "INSERT INTO meal_supplement_doses (meal_id, position, supplement_id, doses) VALUES (?, ?, ?, ?)",
            meal_id, position, supplement_id, doses)
            .execute(&mut *transaction)
            .await?;
    }
    Ok(())
}

/// Every Meal's doses, or one Meal's, in order and by Meal ID, named as the Supplements are now.
pub(super) async fn meal_doses(
    db: &SqlitePool,
    meal_id: Option<&str>,
) -> Result<HashMap<String, Vec<DoseSummary>>> {
    let mut doses: HashMap<String, Vec<DoseSummary>> = HashMap::new();
    for row in sqlx::query!(
        r#"SELECT d.meal_id, d.supplement_id, s.name, s.dose_unit AS "dose_unit: DoseUnit", d.doses
        FROM meal_supplement_doses d JOIN supplements s ON s.id = d.supplement_id
        WHERE ?1 IS NULL OR d.meal_id = ?1 ORDER BY d.meal_id, d.position"#,
        meal_id
    )
    .fetch_all(db)
    .await?
    {
        doses.entry(row.meal_id).or_default().push(DoseSummary {
            supplement_id: row.supplement_id,
            name: row.name,
            dose_unit: row.dose_unit,
            doses: row.doses,
        });
    }
    Ok(doses)
}

/// A logged Supplement Dose: the Supplement as named and labelled when logged, per dose.
#[derive(Debug)]
pub(super) struct StoredDose {
    supplement_id: String,
    name: String,
    dose_unit: DoseUnit,
    pub doses: f64,
    pub amounts: Vec<Amount>,
    pub compounds: Vec<Compound>,
}

impl StoredDose {
    /// A dose to write: `supplement` as it is labelled now.
    pub(super) fn snapshot(supplement: Supplement, doses: f64) -> Self {
        Self {
            supplement_id: supplement.id,
            name: supplement.name,
            dose_unit: supplement.dose_unit,
            doses,
            amounts: supplement.amounts,
            compounds: supplement.compounds,
        }
    }

    /// The dose as shown: its amounts taken, the per-dose amounts times the doses.
    pub(super) fn to_dose(&self) -> SupplementDose {
        let taken = |value: f64| super::meal_logs::round(value * self.doses);
        SupplementDose {
            supplement_id: self.supplement_id.clone(),
            name: self.name.clone(),
            dose_unit: self.dose_unit,
            doses: self.doses,
            amounts: self
                .amounts
                .iter()
                .map(|amount| Amount {
                    value: taken(amount.value),
                    ..amount.clone()
                })
                .collect(),
            compounds: self
                .compounds
                .iter()
                .map(|compound| Compound {
                    value: taken(compound.value),
                    ..compound.clone()
                })
                .collect(),
        }
    }

    pub(super) fn summary(&self) -> DoseSummary {
        DoseSummary {
            supplement_id: self.supplement_id.clone(),
            name: self.name.clone(),
            dose_unit: self.dose_unit,
            doses: self.doses,
        }
    }
}

/// A Supplement Dose of a Meal Log: the Supplement as named when logged, and the amounts taken.
#[derive(Debug, Serialize)]
pub struct SupplementDose {
    /// The Supplement, which may since have been edited or deleted.
    pub supplement_id: String,
    pub name: String,
    pub dose_unit: DoseUnit,
    pub doses: f64,
    /// The label's per-dose amounts as logged, times the doses.
    pub amounts: Vec<Amount>,
    pub compounds: Vec<Compound>,
}

/// The day's doses of each Supplement added together, in the order first taken.
pub(super) fn dose_totals(doses: &[&StoredDose]) -> Vec<DoseSummary> {
    let mut totals: Vec<DoseSummary> = Vec::new();
    for dose in doses {
        match totals
            .iter_mut()
            .find(|total| total.supplement_id == dose.supplement_id)
        {
            Some(total) => total.doses = super::meal_logs::round(total.doses + dose.doses),
            None => totals.push(dose.summary()),
        }
    }
    totals
}

/// Writes a Meal Log's doses with their snapshots; the caller removes the previous doses first.
pub(super) async fn insert_log_doses(
    transaction: &mut sqlx::SqliteConnection,
    meal_log_id: &str,
    doses: &[StoredDose],
) -> Result<()> {
    for (position, dose) in doses.iter().enumerate() {
        let position = position as i64;
        let dose_unit = dose.dose_unit.as_str();
        let dose_id = sqlx::query_scalar!(
            "INSERT INTO meal_log_supplement_doses (meal_log_id, position, supplement_id, name, dose_unit, doses) VALUES (?, ?, ?, ?, ?, ?) RETURNING id",
            meal_log_id, position, dose.supplement_id, dose.name, dose_unit, dose.doses)
            .fetch_one(&mut *transaction)
            .await?;
        for (position, amount) in dose.amounts.iter().enumerate() {
            let position = position as i64;
            sqlx::query!(
                "INSERT INTO meal_log_dose_amounts (dose_id, position, key, form, value, unit) VALUES (?, ?, ?, ?, ?, ?)",
                dose_id, position, amount.key, amount.form, amount.value, amount.unit)
                .execute(&mut *transaction)
                .await?;
        }
        for (position, compound) in dose.compounds.iter().enumerate() {
            let position = position as i64;
            sqlx::query!(
                "INSERT INTO meal_log_dose_compounds (dose_id, position, name, value, unit) VALUES (?, ?, ?, ?, ?)",
                dose_id, position, compound.name, compound.value, compound.unit)
                .execute(&mut *transaction)
                .await?;
        }
    }
    Ok(())
}

/// The doses of one Meal Log (`id`) or of the Meal Logs on days `from` to `to`, in order and by
/// Meal Log ID.
pub(super) async fn log_doses(
    db: &SqlitePool,
    id: Option<&str>,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<HashMap<String, Vec<StoredDose>>> {
    let rows = sqlx::query!(
        r#"SELECT d.id AS "id!", d.meal_log_id, d.supplement_id, d.name,
            d.dose_unit AS "dose_unit: DoseUnit", d.doses
        FROM meal_log_supplement_doses d JOIN meal_logs m ON m.id = d.meal_log_id
        WHERE (?1 IS NULL OR m.id = ?1) AND (?2 IS NULL OR m.day >= ?2) AND (?3 IS NULL OR m.day <= ?3)
        ORDER BY d.meal_log_id, d.position"#,
        id,
        from,
        to
    )
    .fetch_all(db)
    .await?;
    if rows.is_empty() {
        return Ok(HashMap::new());
    }
    let mut amounts: HashMap<i64, Vec<Amount>> = HashMap::new();
    for row in sqlx::query!(
        r#"SELECT a.dose_id, a.key, a.form, a.value, a.unit FROM meal_log_dose_amounts a
        JOIN meal_log_supplement_doses d ON d.id = a.dose_id JOIN meal_logs m ON m.id = d.meal_log_id
        WHERE (?1 IS NULL OR m.id = ?1) AND (?2 IS NULL OR m.day >= ?2) AND (?3 IS NULL OR m.day <= ?3)
        ORDER BY a.dose_id, a.position"#,
        id,
        from,
        to
    )
    .fetch_all(db)
    .await?
    {
        amounts.entry(row.dose_id).or_default().push(Amount {
            key: row.key,
            form: row.form,
            value: row.value,
            unit: row.unit,
        });
    }
    let mut compounds: HashMap<i64, Vec<Compound>> = HashMap::new();
    for row in sqlx::query!(
        r#"SELECT c.dose_id, c.name, c.value, c.unit FROM meal_log_dose_compounds c
        JOIN meal_log_supplement_doses d ON d.id = c.dose_id JOIN meal_logs m ON m.id = d.meal_log_id
        WHERE (?1 IS NULL OR m.id = ?1) AND (?2 IS NULL OR m.day >= ?2) AND (?3 IS NULL OR m.day <= ?3)
        ORDER BY c.dose_id, c.position"#,
        id,
        from,
        to
    )
    .fetch_all(db)
    .await?
    {
        compounds.entry(row.dose_id).or_default().push(Compound {
            name: row.name,
            value: row.value,
            unit: row.unit,
        });
    }
    let mut doses: HashMap<String, Vec<StoredDose>> = HashMap::new();
    for row in rows {
        doses.entry(row.meal_log_id).or_default().push(StoredDose {
            amounts: amounts.remove(&row.id).unwrap_or_default(),
            compounds: compounds.remove(&row.id).unwrap_or_default(),
            supplement_id: row.supplement_id,
            name: row.name,
            dose_unit: row.dose_unit,
            doses: row.doses,
        });
    }
    Ok(doses)
}

/// Converts `value` of a compound from one mass unit to another; `None` for other units.
pub(super) fn convert_mass(value: f64, from: &str, to: &str) -> Option<f64> {
    let (_, from) = mass_unit(from)?;
    let (_, to) = mass_unit(to)?;
    Some(value * to / from)
}
