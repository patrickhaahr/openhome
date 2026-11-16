use super::{Amount, Compound, DoseUnit, Result, Supplement, SupplementError, stored};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::collections::HashMap;

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
pub(in crate::services::nutrition) fn check_doses(doses: f64) -> std::result::Result<(), String> {
    if !doses.is_finite() || doses <= 0.0 || doses > MAX_DOSES {
        return Err(format!(
            "doses must be above 0 and at most {MAX_DOSES} of the Supplement's dose unit"
        ));
    }
    Ok(())
}

/// Parses up to [`MAX_SUPPLEMENT_DOSES`] doses, each Supplement listed once.
pub(in crate::services::nutrition) fn parse_doses(
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
pub(in crate::services::nutrition) fn normalized(
    supplement_id: &str,
    doses: f64,
) -> serde_json::Value {
    serde_json::json!({"supplement_id": supplement_id, "doses": doses})
}

fn unknown(id: &str) -> SupplementError {
    SupplementError::NotFound(format!(
        "Supplement {id} not found; list supplements for its id, or add_supplement it from its \
         label first"
    ))
}

/// The Supplements `doses` name, in order; any unknown Supplement is [`SupplementError::NotFound`].
pub(in crate::services::nutrition) async fn resolve(
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
#[derive(Debug, Serialize, Deserialize)]
pub struct DoseSummary {
    pub supplement_id: String,
    /// The Supplement as it is named now in a Meal, or as it was logged in a day.
    pub name: String,
    pub dose_unit: DoseUnit,
    pub doses: f64,
}

/// Writes a Meal's doses, replacing none: the caller removes the previous doses first.
pub(in crate::services::nutrition) async fn insert_meal_doses(
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
pub(in crate::services::nutrition) async fn meal_doses(
    db: &mut sqlx::SqliteConnection,
    meal_id: Option<&str>,
) -> Result<HashMap<String, Vec<DoseSummary>>> {
    let mut doses: HashMap<String, Vec<DoseSummary>> = HashMap::new();
    for row in sqlx::query!(
        r#"SELECT d.meal_id, d.supplement_id, s.name, s.dose_unit AS "dose_unit: DoseUnit", d.doses
        FROM meal_supplement_doses d JOIN supplements s ON s.id = d.supplement_id
        WHERE ?1 IS NULL OR d.meal_id = ?1 ORDER BY d.meal_id, d.position"#,
        meal_id
    )
    .fetch_all(&mut *db)
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
pub(in crate::services::nutrition) struct StoredDose {
    supplement_id: String,
    name: String,
    dose_unit: DoseUnit,
    pub doses: f64,
    pub amounts: Vec<Amount>,
    pub compounds: Vec<Compound>,
}

impl StoredDose {
    /// A dose to write: `supplement` as it is labelled now.
    pub(in crate::services::nutrition) fn snapshot(supplement: Supplement, doses: f64) -> Self {
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
    pub(in crate::services::nutrition) fn to_dose(&self) -> SupplementDose {
        let taken = |value: f64| crate::services::nutrition::meal_logs::round(value * self.doses);
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

    pub(in crate::services::nutrition) fn summary(&self) -> DoseSummary {
        DoseSummary {
            supplement_id: self.supplement_id.clone(),
            name: self.name.clone(),
            dose_unit: self.dose_unit,
            doses: self.doses,
        }
    }
}

/// A Supplement Dose of a Meal Log: the Supplement as named when logged, and the amounts taken.
#[derive(Debug, Serialize, Deserialize)]
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
pub(in crate::services::nutrition) fn dose_totals(doses: &[&StoredDose]) -> Vec<DoseSummary> {
    let mut totals: Vec<DoseSummary> = Vec::new();
    for dose in doses {
        match totals
            .iter_mut()
            .find(|total| total.supplement_id == dose.supplement_id)
        {
            Some(total) => {
                total.doses =
                    crate::services::nutrition::meal_logs::round(total.doses + dose.doses);
            }
            None => totals.push(dose.summary()),
        }
    }
    totals
}

/// Writes a Meal Log's doses with their snapshots; the caller removes the previous doses first.
pub(in crate::services::nutrition) async fn insert_log_doses(
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
pub(in crate::services::nutrition) async fn log_doses(
    db: &SqlitePool,
    id: Option<&str>,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<HashMap<String, Vec<StoredDose>>> {
    log_doses_on(&mut *db.acquire().await?, id, from, to).await
}

pub(in crate::services::nutrition) async fn log_doses_on(
    db: &mut sqlx::SqliteConnection,
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
    .fetch_all(&mut *db)
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
    .fetch_all(&mut *db)
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
    .fetch_all(&mut *db)
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
