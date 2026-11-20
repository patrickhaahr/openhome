use super::{
    AddSupplement, Added, Amount, Compound, DoseUnit, EditSupplement, Result, Supplement,
    SupplementError, Supplements, amounts, check_label, compounds, validation,
};
use crate::services::nutrition::meal_logs::parse_request_id;
use crate::services::nutrition::meals::UniqueName;
use crate::services::patch::Patch;
use sqlx::SqlitePool;
use std::collections::HashMap;
use uuid::Uuid;

async fn load(db: &SqlitePool, id: Option<&str>) -> Result<Vec<Supplement>> {
    load_on(&mut *db.acquire().await?, id).await
}

async fn load_on(db: &mut sqlx::SqliteConnection, id: Option<&str>) -> Result<Vec<Supplement>> {
    let rows = sqlx::query!(
        r#"SELECT id AS "id!", name, dose_unit AS "dose_unit: DoseUnit" FROM supplements
        WHERE ?1 IS NULL OR id = ?1 ORDER BY name_key"#,
        id
    )
    .fetch_all(&mut *db)
    .await?;
    let mut amounts: HashMap<String, Vec<Amount>> = HashMap::new();
    for row in sqlx::query!(
        "SELECT supplement_id, key, form, value, unit FROM supplement_amounts
        WHERE ?1 IS NULL OR supplement_id = ?1 ORDER BY supplement_id, position",
        id
    )
    .fetch_all(&mut *db)
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
    .fetch_all(&mut *db)
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

pub(super) async fn find(db: &SqlitePool, id: &str) -> Result<Option<Supplement>> {
    Ok(load(db, Some(id)).await?.into_iter().next())
}

fn not_found() -> SupplementError {
    SupplementError::NotFound("Supplement not found; list supplements for its id".into())
}

pub(in crate::services::nutrition) async fn stored(
    db: &SqlitePool,
    ids: &[&str],
) -> Result<HashMap<String, Supplement>> {
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

pub async fn supplements(db: &SqlitePool) -> Result<Supplements> {
    Ok(Supplements {
        supplements: load(db, None).await?,
    })
}

async fn replay(db: &SqlitePool, request_id: &str, request: &str) -> Result<Option<Added>> {
    let Some(receipt) = sqlx::query!(
        "SELECT supplement_id, request, result FROM supplement_receipts WHERE request_id = ?",
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
    find(db, &receipt.supplement_id).await?.ok_or_else(|| {
        SupplementError::Conflict(
            "The retried Supplement was deleted; use a new request_id to add it again".into(),
        )
    })?;
    let result = receipt.result.ok_or_else(|| SupplementError::Conflict(
        "This legacy receipt has no original result; read the current supplement before retrying".into()))?;
    let supplement =
        serde_json::from_str(&result).map_err(|error| sqlx::Error::Decode(Box::new(error)))?;
    Ok(Some(Added {
        supplement,
        replayed: true,
    }))
}

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
    let supplement = load_on(&mut transaction, Some(&id))
        .await?
        .into_iter()
        .next()
        .ok_or_else(not_found)?;
    let result =
        serde_json::to_string(&supplement).map_err(|error| sqlx::Error::Encode(Box::new(error)))?;
    sqlx::query!(
        "UPDATE supplement_receipts SET result = ? WHERE request_id = ?",
        result,
        request_id
    )
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(Added {
        supplement,
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
