use std::num::NonZeroU32;

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use uuid::Uuid;

use super::card_value::{BinderValue, Unpriced, Valuation};
use super::cards::{self, Card, CardId, CardsError};
use super::optcg::Optcg;
use super::patch::Patch;
use super::training::CalendarDay;
use super::wishlist::HouseholdUser;

#[derive(Debug, thiserror::Error)]
pub enum BinderError {
    #[error("{0}")]
    Validation(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Conflict(String),
    #[error("Which household owner's Binder should receive this card?")]
    DestinationRequired { choices: Vec<HouseholdUser> },
    #[error("The catalogue does not list this card")]
    NotListed,
    #[error(transparent)]
    Cards(#[from] CardsError),
    #[error("Binder database operation failed")]
    Database(#[from] sqlx::Error),
}

type Result<T> = std::result::Result<T, BinderError>;

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, sqlx::Type,
)]
#[serde(rename_all = "lowercase")]
#[sqlx(rename_all = "lowercase")]
pub enum Language {
    #[default]
    En,
    Ja,
}

/// A copy count of at least 1. `u32` keeps a sum of additions inside SQLite's `i64`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Quantity(NonZeroU32);

impl Quantity {
    fn parse(value: i64) -> Result<Self> {
        u32::try_from(value)
            .ok()
            .and_then(NonZeroU32::new)
            .map(Self)
            .ok_or_else(|| {
                BinderError::Validation(format!(
                    "quantity must be an integer from 1 to {}",
                    u32::MAX
                ))
            })
    }

    fn get(self) -> i64 {
        i64::from(self.0.get())
    }
}

/// A Binder is only its owner's entries: there is no Binder record.
#[derive(Debug, Serialize, Deserialize)]
pub struct BinderEntry {
    pub id: String,
    pub owner_id: String,
    pub card: Card,
    pub language: Language,
    pub quantity: i64,
    pub note: Option<String>,
    /// The Europe/Copenhagen calendar day the entry was first added.
    pub date_added: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AddEntry {
    pub owner_id: Option<String>,
    pub card_id: String,
    pub quantity: Option<i64>,
    pub language: Option<Language>,
    pub note: Option<String>,
    pub request_id: String,
}

#[derive(Serialize)]
pub struct Addition {
    pub entry: BinderEntry,
    pub replayed: bool,
}

struct Checked {
    owner_id: Option<String>,
    card_id: CardId,
    quantity: Quantity,
    language: Language,
    note: Option<String>,
    request_id: String,
}

fn text(value: &str, field: &str, max: usize) -> Result<String> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > max || value.chars().any(char::is_control) {
        return Err(BinderError::Validation(format!(
            "{field} must contain 1-{max} characters without control characters"
        )));
    }
    Ok(value.to_owned())
}

impl Checked {
    fn parse(request: AddEntry) -> Result<Self> {
        Ok(Self {
            owner_id: request.owner_id,
            card_id: CardId::parse(&request.card_id)?,
            quantity: Quantity::parse(request.quantity.unwrap_or(1))?,
            language: request.language.unwrap_or_default(),
            note: request
                .note
                .map(|note| text(&note, "note", 500))
                .transpose()?,
            request_id: text(&request.request_id, "request_id", 128)?,
        })
    }
}

struct Receipt {
    entry_id: String,
    owner_id: String,
    card_id: String,
    language: Language,
    quantity: i64,
    note: Option<String>,
    result: String,
}

impl Receipt {
    async fn replay(self, db: &SqlitePool, request: &Checked) -> Result<Addition> {
        let same = request.owner_id.as_deref() == Some(self.owner_id.as_str())
            && request.card_id.as_str() == self.card_id
            && request.language == self.language
            && request.quantity.get() == self.quantity
            && request.note == self.note;
        if !same {
            return Err(BinderError::Conflict(
                "request_id was already used for a different addition; use a new ID for a new request"
                    .into(),
            ));
        }
        let removed = sqlx::query_scalar!(
            r#"SELECT 1 AS "found!: i64" FROM binder_entries WHERE id = ? AND owner_id = ?"#,
            self.entry_id,
            self.owner_id
        )
        .fetch_optional(db)
        .await?
        .is_none();
        if removed {
            return Err(BinderError::Conflict(
                "The retried entry was removed; use a new request_id to add it again".into(),
            ));
        }
        let entry = serde_json::from_str(&self.result)
            .map_err(|error| sqlx::Error::Decode(Box::new(error)))?;
        Ok(Addition {
            entry,
            replayed: true,
        })
    }
}

async fn previous(db: &SqlitePool, request_id: &str) -> Result<Option<Receipt>> {
    Ok(sqlx::query_as!(
        Receipt,
        r#"SELECT entry_id, owner_id, card_id, language AS "language: Language", quantity, note, result
           FROM binder_addition_receipts WHERE request_id = ?"#,
        request_id
    )
    .fetch_optional(db)
    .await?)
}

pub async fn add_entry(db: &SqlitePool, catalogue: &Optcg, request: AddEntry) -> Result<Addition> {
    let request = Checked::parse(request)?;
    // Resolve accepted retries before discovery, so a removed owner cannot revive an addition.
    if let Some(receipt) = previous(db, &request.request_id).await? {
        return receipt.replay(db, &request).await;
    }
    let Some(owner_id) = request.owner_id.as_deref() else {
        let choices = sqlx::query_as!(
            HouseholdUser,
            "SELECT id, name FROM household_users ORDER BY name, id"
        )
        .fetch_all(db)
        .await?;
        return Err(BinderError::DestinationRequired { choices });
    };
    if sqlx::query!("SELECT id FROM household_users WHERE id = ?", owner_id)
        .fetch_optional(db)
        .await?
        .is_none()
    {
        return Err(BinderError::NotFound(
            "Household User not found; discover household users first".into(),
        ));
    }
    let card = cards::listed(db, catalogue, &request.card_id)
        .await?
        .ok_or(BinderError::NotListed)?;
    let id = Uuid::new_v4().to_string();
    let date_added = CalendarDay::of(chrono::Utc::now()).to_string();
    let quantity = request.quantity.get();
    let card_id = request.card_id.as_str();
    let mut tx = db.begin().await?;
    // A supplied note replaces the stored one; an omitted note keeps it.
    let stored = sqlx::query!(
        r#"INSERT INTO binder_entries (id, owner_id, card_id, language, quantity, note, date_added)
           VALUES (?, ?, ?, ?, ?, ?, ?)
           ON CONFLICT (owner_id, card_id, language) DO UPDATE
           SET quantity = quantity + excluded.quantity, note = COALESCE(excluded.note, note)
           RETURNING id AS "id!", quantity AS "quantity!", note AS "note?", date_added AS "date_added!""#,
        id,
        owner_id,
        card_id,
        request.language,
        quantity,
        request.note,
        date_added
    )
    .fetch_one(&mut *tx)
    .await?;
    let entry = BinderEntry {
        id: stored.id,
        owner_id: owner_id.to_owned(),
        card: priced_for(card, request.language),
        language: request.language,
        quantity: stored.quantity,
        note: stored.note,
        date_added: stored.date_added,
    };
    let result =
        serde_json::to_string(&entry).map_err(|error| sqlx::Error::Encode(Box::new(error)))?;
    // The receipt claim serializes simultaneous retries; the loser rolls back its merge.
    let claimed = sqlx::query_scalar!(
        "INSERT INTO binder_addition_receipts
            (request_id, entry_id, owner_id, card_id, language, quantity, note, result)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT (request_id) DO NOTHING RETURNING request_id",
        request.request_id,
        entry.id,
        owner_id,
        card_id,
        request.language,
        quantity,
        request.note,
        result
    )
    .fetch_optional(&mut *tx)
    .await?;
    if claimed.is_none() {
        tx.rollback().await?;
        return previous(db, &request.request_id)
            .await?
            .ok_or_else(|| {
                BinderError::Conflict(
                    "Retry receipt unavailable; refresh before trying again".into(),
                )
            })?
            .replay(db, &request)
            .await;
    }
    tx.commit().await?;
    Ok(Addition {
        entry,
        replayed: false,
    })
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntriesFilter {
    pub owner_id: Option<String>,
}

/// One owner's Binder Value, for an owner that has entries in the list.
#[derive(Debug, Serialize)]
pub struct OwnerValue {
    pub owner_id: String,
    #[serde(flatten)]
    pub value: BinderValue,
}

#[derive(Debug, Serialize)]
pub struct Entries {
    pub entries: Vec<BinderEntry>,
    pub binder_values: Vec<OwnerValue>,
}

pub async fn entries(db: &SqlitePool, filter: EntriesFilter) -> Result<Entries> {
    let entries = load(db, filter.owner_id.as_deref(), None).await?;
    // Entries are grouped by owner, so each run of one owner is one Binder.
    let binder_values = entries
        .chunk_by(|a, b| a.owner_id == b.owner_id)
        .map(|binder| OwnerValue {
            owner_id: binder[0].owner_id.clone(),
            value: BinderValue::total(
                binder
                    .iter()
                    .map(|entry| (entry.quantity, &entry.card.valuation)),
            ),
        })
        .collect();
    Ok(Entries {
        entries,
        binder_values,
    })
}

/// Read only the complete owner/entry destination.
pub async fn entry(db: &SqlitePool, owner_id: &str, id: &str) -> Result<BinderEntry> {
    load(db, Some(owner_id), Some(id))
        .await?
        .pop()
        .ok_or_else(not_found)
}

fn not_found() -> BinderError {
    BinderError::NotFound("Binder Entry not found for this Household User".into())
}

async fn load(
    db: &SqlitePool,
    owner_id: Option<&str>,
    id: Option<&str>,
) -> Result<Vec<BinderEntry>> {
    let mut rows = sqlx::query!(
        r#"SELECT e.id, e.owner_id, e.card_id, e.language AS "language: Language", e.quantity,
                  e.note, e.date_added, u.name AS owner_name, c.card_code, c.position
           FROM binder_entries e
           JOIN household_users u ON u.id = e.owner_id
           JOIN cards c ON c.id = e.card_id
           WHERE (? IS NULL OR e.owner_id = ?) AND (? IS NULL OR e.id = ?)"#,
        owner_id,
        owner_id,
        id,
        id
    )
    .fetch_all(db)
    .await?;
    rows.sort_by_cached_key(|row| {
        (
            row.owner_name.clone(),
            row.owner_id.clone(),
            card_code_order(&row.card_code, &row.card_id, row.position),
            row.language,
        )
    });
    let ids: Vec<&str> = rows.iter().map(|row| row.card_id.as_str()).collect();
    let cards = cards::by_ids(db, &ids).await?;
    rows.into_iter()
        .map(|row| {
            Ok(BinderEntry {
                card: priced_for(
                    cards
                        .get(&row.card_id)
                        .cloned()
                        .ok_or(sqlx::Error::RowNotFound)?,
                    row.language,
                ),
                id: row.id,
                owner_id: row.owner_id,
                language: row.language,
                quantity: row.quantity,
                note: row.note,
                date_added: row.date_added,
            })
        })
        .collect()
}

/// Absent fields keep their stored value and a null note clears it. Quantity and language have
/// no empty state, so null for either is rejected.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EditEntry {
    #[serde(default)]
    pub quantity: Patch<i64>,
    #[serde(default)]
    pub language: Patch<Language>,
    #[serde(default)]
    pub note: Patch<String>,
}

fn required<T>(patch: Patch<T>, field: &str) -> Result<Option<T>> {
    match patch {
        Patch::Missing => Ok(None),
        Patch::Value(Some(value)) => Ok(Some(value)),
        Patch::Value(None) => Err(BinderError::Validation(format!("{field} cannot be null"))),
    }
}

/// A language that another entry of the same owner and Card already holds returns 409 and the
/// whole correction is rolled back, so entries are never merged silently.
pub async fn edit_entry(
    db: &SqlitePool,
    owner_id: &str,
    id: &str,
    request: EditEntry,
) -> Result<BinderEntry> {
    let quantity = required(request.quantity, "quantity")?
        .map(Quantity::parse)
        .transpose()?
        .map(Quantity::get);
    let language = required(request.language, "language")?;
    let (note_given, note) = match request.note {
        Patch::Missing => (false, None),
        Patch::Value(note) => (true, note.map(|note| text(&note, "note", 500)).transpose()?),
    };
    let updated = sqlx::query_scalar!(
        r#"UPDATE binder_entries
           SET quantity = COALESCE(?, quantity), language = COALESCE(?, language),
               note = CASE WHEN ? THEN ? ELSE note END
           WHERE id = ? AND owner_id = ? RETURNING id AS "id!""#,
        quantity,
        language,
        note_given,
        note,
        id,
        owner_id
    )
    .fetch_optional(db)
    .await
    .map_err(|error| match error {
        sqlx::Error::Database(error) if error.is_unique_violation() => BinderError::Conflict(
            "This owner already has the Card in that language; raise its quantity instead".into(),
        ),
        other => other.into(),
    })?;
    if updated.is_none() {
        return Err(not_found());
    }
    entry(db, owner_id, id).await
}

/// Deletion is final. The accepted addition receipt stays retired.
pub async fn remove_entry(db: &SqlitePool, owner_id: &str, id: &str) -> Result<()> {
    let removed = sqlx::query!(
        "DELETE FROM binder_entries WHERE id = ? AND owner_id = ?",
        id,
        owner_id
    )
    .execute(db)
    .await?;
    if removed.rows_affected() == 0 {
        return Err(not_found());
    }
    Ok(())
}

/// The catalogue's prices are for English printings, so a Japanese entry has no price at all.
fn priced_for(mut card: Card, language: Language) -> Card {
    if language == Language::Ja {
        card.valuation = Valuation::Unpriced(Unpriced::JapanesePrinting);
    }
    card
}

/// A normal print (its id is the Card Code) precedes its parallels, which keep catalogue order.
/// Prefixes compare alphabetically, so a new set slots in without a table to maintain.
fn card_code_order(card_code: &str, card_id: &str, position: i64) -> (String, u32, u32, bool, i64) {
    let (set, number) = card_code.split_once('-').unwrap_or((card_code, ""));
    let letters = set.bytes().take_while(u8::is_ascii_alphabetic).count();
    let (prefix, set_number) = set.split_at(letters);
    (
        prefix.to_owned(),
        set_number.parse().unwrap_or(0),
        number.parse().unwrap_or(0),
        card_id != card_code,
        position,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn card_code_order_reads_prefix_then_set_then_card_with_parallels_after_the_normal_print() {
        let in_order = [
            ("EB01-061", "EB01-061", 0),
            ("EB03-002", "EB03-002", 0),
            ("OP01-002", "OP01-002", 0),
            ("OP01-010", "OP01-010", 0),
            ("OP01-010", "OP01-010_p1", 1),
            ("OP01-010", "OP01-010_p2", 2),
            ("OP02-001", "OP02-001", 0),
            ("OP09-034", "OP09-034", 1),
            ("OP09-034", "EB03_OP09-034_p1", 0),
            ("OP10-001", "OP10-001", 0),
            ("P-001", "P-001", 0),
            ("P-010", "P-010", 0),
            ("PRB01-001", "PRB01-001", 0),
            ("ST01-001", "ST01-001", 0),
            ("ST10-002", "ST10-002", 0),
        ];
        let mut shuffled = in_order;
        shuffled.reverse();
        shuffled.sort_by_key(|(code, id, position)| card_code_order(code, id, *position));
        assert_eq!(shuffled.map(|(_, id, _)| id), in_order.map(|(_, id, _)| id));
    }
}
