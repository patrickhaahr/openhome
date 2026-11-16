use super::{
    KEY_NUTRIENTS, MAX_RANGE_DAYS, MealLog, MealLogError, MealLogs, MealLogsQuery, PortionProduct,
    Result, StoredLog, StoredPhoto, StoredPortion, StoredValue, ValueSource, parse_day,
};
use crate::services::nutrition::nova::NovaClass;
use crate::services::nutrition::supplements;
use crate::services::training::calendar::{self, CalendarDay};
use sqlx::SqlitePool;
use std::collections::HashMap;

/// Which Meal Logs to load.
pub(in crate::services::nutrition) enum Selection<'a> {
    One(&'a str),
    Days(CalendarDay, CalendarDay),
}

/// One snapshotted Portion value as read.
struct ValueRow {
    portion_id: i64,
    key: String,
    value: f64,
    unit: String,
    source: ValueSource,
}

/// Which snapshotted Portion values to load.
#[derive(Clone, Copy, Debug)]
pub(in crate::services::nutrition) enum Values<'a> {
    All,
    /// Only these keys, so a long range never reads the hundreds of values it does not use.
    Only(&'a [&'a str]),
}

/// Loads Meal Logs in time order with their Portions and Supplement Doses, with the Portion
/// values `values` selects. Doses keep every amount.
pub(in crate::services::nutrition) async fn load(
    db: &SqlitePool,
    selection: &Selection<'_>,
    values: Values<'_>,
) -> Result<Vec<StoredLog>> {
    load_on(&mut *db.acquire().await?, selection, values).await
}

pub(in crate::services::nutrition) async fn load_on(
    db: &mut sqlx::SqliteConnection,
    selection: &Selection<'_>,
    values: Values<'_>,
) -> Result<Vec<StoredLog>> {
    let (id, from, to) = match selection {
        Selection::One(id) => (Some((*id).to_owned()), None, None),
        Selection::Days(from, to) => (None, Some(from.to_string()), Some(to.to_string())),
    };
    let logs = sqlx::query!(
        r#"SELECT id AS "id!", eaten_at, day, meal_name, note, photo_id, photo_width, photo_height,
            photo_thumbnail_width, photo_thumbnail_height FROM meal_logs
        WHERE (?1 IS NULL OR id = ?1) AND (?2 IS NULL OR day >= ?2) AND (?3 IS NULL OR day <= ?3)
        ORDER BY eaten_at, created_at, id"#,
        id,
        from,
        to
    )
    .fetch_all(&mut *db)
    .await?;
    let portions = sqlx::query!(
        r#"SELECT p.id AS "id!", p.meal_log_id, p.grams, p.food_id, p.name_da, p.name_en, p.dataset_version,
            p.product_barcode, p.product_name, p.product_brand, p.nova_class
        FROM meal_log_portions p JOIN meal_logs m ON m.id = p.meal_log_id
        WHERE (?1 IS NULL OR m.id = ?1) AND (?2 IS NULL OR m.day >= ?2) AND (?3 IS NULL OR m.day <= ?3)
        ORDER BY p.meal_log_id, p.position"#,
        id,
        from,
        to
    )
    .fetch_all(&mut *db)
    .await?;
    // Driven from the Meal Logs to each Portion's values by primary key, so a range reads only
    // its own Portions' values, and for a key list one seek per Portion and key.
    let values: Vec<ValueRow> = match values {
        Values::All => {
            sqlx::query_as!(
                ValueRow,
                r#"SELECT v.portion_id AS "portion_id!", v.key AS "key!", v.value AS "value!",
                    v.unit AS "unit!", v.source AS "source!: ValueSource"
                FROM meal_logs m CROSS JOIN meal_log_portions p ON p.meal_log_id = m.id
                CROSS JOIN meal_log_portion_values v ON v.portion_id = p.id
                WHERE (?1 IS NULL OR m.id = ?1) AND (?2 IS NULL OR m.day >= ?2)
                    AND (?3 IS NULL OR m.day <= ?3)"#,
                id,
                from,
                to
            )
            .fetch_all(&mut *db)
            .await?
        }
        Values::Only(keys) => {
            let keys = serde_json::Value::from(keys).to_string();
            sqlx::query_as!(
                ValueRow,
                r#"WITH k (key) AS MATERIALIZED (SELECT value FROM json_each(?4))
                SELECT v.portion_id AS "portion_id!", v.key AS "key!", v.value AS "value!",
                    v.unit AS "unit!", v.source AS "source!: ValueSource"
                FROM meal_logs m CROSS JOIN meal_log_portions p ON p.meal_log_id = m.id
                CROSS JOIN k CROSS JOIN meal_log_portion_values v
                    ON v.portion_id = p.id AND v.key = k.key
                WHERE (?1 IS NULL OR m.id = ?1) AND (?2 IS NULL OR m.day >= ?2)
                    AND (?3 IS NULL OR m.day <= ?3)"#,
                id,
                from,
                to,
                keys
            )
            .fetch_all(&mut *db)
            .await?
        }
    };

    let mut doses =
        supplements::log_doses_on(db, id.as_deref(), from.as_deref(), to.as_deref()).await?;

    let mut values_by_portion: HashMap<i64, HashMap<String, StoredValue>> = HashMap::new();
    for row in values {
        values_by_portion.entry(row.portion_id).or_default().insert(
            row.key,
            StoredValue {
                value: row.value,
                unit: row.unit,
                source: row.source,
            },
        );
    }
    let mut portions_by_log: HashMap<String, Vec<StoredPortion>> = HashMap::new();
    for row in portions {
        portions_by_log
            .entry(row.meal_log_id)
            .or_default()
            .push(StoredPortion {
                food_id: row.food_id,
                product: row.product_barcode.map(|barcode| PortionProduct {
                    barcode,
                    name: row.product_name,
                    brand: row.product_brand,
                }),
                name_da: row.name_da,
                name_en: row.name_en,
                grams: row.grams,
                dataset_version: row.dataset_version,
                nova_class: row.nova_class.and_then(NovaClass::new),
                values: values_by_portion.remove(&row.id).unwrap_or_default(),
            });
    }
    Ok(logs
        .into_iter()
        .map(|row| StoredLog {
            portions: portions_by_log.remove(&row.id).unwrap_or_default(),
            doses: doses.remove(&row.id).unwrap_or_default(),
            photo: StoredPhoto::from_columns(
                row.photo_id,
                row.photo_width,
                row.photo_height,
                row.photo_thumbnail_width,
                row.photo_thumbnail_height,
            ),
            id: row.id,
            eaten_at: row.eaten_at,
            day: row.day,
            meal_name: row.meal_name,
            note: row.note,
        })
        .collect())
}

/// One Meal Log, or `None` when it does not exist.
pub(super) async fn find(db: &SqlitePool, id: &str) -> Result<Option<MealLog>> {
    load(db, &Selection::One(id), Values::Only(&KEY_NUTRIENTS))
        .await?
        .into_iter()
        .next()
        .map(StoredLog::into_meal_log)
        .transpose()
}

pub(super) fn not_found() -> MealLogError {
    MealLogError::NotFound("Meal Log not found; list meal_logs for its id".into())
}

/// Meal Logs of one day, or of an inclusive range of up to [`MAX_RANGE_DAYS`] days, in time
/// order.
pub async fn meal_logs(db: &SqlitePool, query: &MealLogsQuery) -> Result<MealLogs> {
    let (from, to) = match (&query.day, &query.from, &query.to) {
        (Some(day), None, None) => {
            let day = parse_day(day, "day")?;
            (day, day)
        }
        (None, Some(from), Some(to)) => {
            let (from, to) = (parse_day(from, "from")?, parse_day(to, "to")?);
            if !(0..MAX_RANGE_DAYS).contains(&from.days_until(to)) {
                return Err(MealLogError::Validation(format!(
                    "from and to must cover 1 to {MAX_RANGE_DAYS} calendar days, with from <= to"
                )));
            }
            (from, to)
        }
        _ => {
            return Err(MealLogError::Validation(
                "give either day, or both from and to (YYYY-MM-DD)".into(),
            ));
        }
    };
    let meal_logs = load(db, &Selection::Days(from, to), Values::Only(&KEY_NUTRIENTS))
        .await?
        .into_iter()
        .map(StoredLog::into_meal_log)
        .collect::<Result<_>>()?;
    Ok(MealLogs {
        from: from.to_string(),
        to: to.to_string(),
        time_zone: calendar::TIME_ZONE_NAME,
        meal_logs,
    })
}
