//! Household-visible movie and whole-series plans, with explicit owners and durable retry identity.

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use uuid::Uuid;

use super::wishlist::HouseholdUser;

#[derive(Debug, thiserror::Error)]
pub enum WatchlistError {
    #[error("{0}")]
    Validation(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Conflict(String),
    #[error("Which household owner's watchlist should receive this entry?")]
    DestinationRequired { choices: Vec<HouseholdUser> },
    #[error("Watchlist database operation failed")]
    Database(#[from] sqlx::Error),
}

type Result<T> = std::result::Result<T, WatchlistError>;

/// A whole media title, never an episode or season.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, sqlx::Type)]
#[serde(rename_all = "lowercase")]
#[sqlx(rename_all = "lowercase")]
pub enum MediaKind {
    Movie,
    Series,
}

/// This slice creates plans only; transitions are delivered in #102.
#[derive(Debug, Serialize, sqlx::Type)]
#[serde(rename_all = "lowercase")]
#[sqlx(rename_all = "lowercase")]
pub enum WatchStatus {
    Planned,
}

#[derive(Debug, Serialize)]
pub struct WatchlistEntry {
    pub id: String,
    pub owner_id: String,
    pub kind: MediaKind,
    pub title: String,
    pub status: WatchStatus,
    pub date_added: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AddEntry {
    pub owner_id: Option<String>,
    pub kind: MediaKind,
    pub title: String,
    pub request_id: String,
}

#[derive(Serialize)]
pub struct Addition {
    pub entry: WatchlistEntry,
    pub replayed: bool,
}

struct AdditionReceipt {
    entry_id: String,
    owner_id: String,
    kind: MediaKind,
    title: String,
}

impl AdditionReceipt {
    async fn replay(self, db: &SqlitePool, request: &AddEntry) -> Result<Addition> {
        if request.owner_id.as_deref() != Some(self.owner_id.as_str())
            || request.kind != self.kind
            || request.title != self.title
        {
            return Err(WatchlistError::Conflict(
                "request_id was already used for a different addition; use a new ID for a new request".into(),
            ));
        }
        let entry =
            entry(db, &self.owner_id, &self.entry_id)
                .await
                .map_err(|error| match error {
                    WatchlistError::NotFound(_) => WatchlistError::Conflict(
                        "The retried entry was removed; use a new request_id to add it again"
                            .into(),
                    ),
                    other => other,
                })?;
        Ok(Addition {
            entry,
            replayed: true,
        })
    }
}

fn text(value: &str, field: &str, max: usize) -> Result<String> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > max || value.chars().any(char::is_control) {
        return Err(WatchlistError::Validation(format!(
            "{field} must contain 1-{max} characters without control characters"
        )));
    }
    Ok(value.to_owned())
}

/// Shared read across all configured Household Users, including independent same-title plans.
pub async fn entries(db: &SqlitePool) -> Result<Vec<WatchlistEntry>> {
    Ok(sqlx::query_as!(WatchlistEntry,
        "SELECT id, owner_id, kind AS \"kind: MediaKind\", title, status AS \"status: WatchStatus\", date_added FROM watchlist_entries ORDER BY date_added DESC, id")
        .fetch_all(db).await?)
}

/// Read only the complete owner/entry destination.
pub async fn entry(db: &SqlitePool, owner_id: &str, id: &str) -> Result<WatchlistEntry> {
    sqlx::query_as!(WatchlistEntry,
        "SELECT id, owner_id, kind AS \"kind: MediaKind\", title, status AS \"status: WatchStatus\", date_added FROM watchlist_entries WHERE id = ? AND owner_id = ?", id, owner_id)
        .fetch_optional(db).await?
        .ok_or_else(|| WatchlistError::NotFound("Watchlist Entry not found for this Household User".into()))
}

async fn previous(db: &SqlitePool, request_id: &str) -> Result<Option<AdditionReceipt>> {
    Ok(sqlx::query_as!(AdditionReceipt,
        "SELECT entry_id, owner_id, kind AS \"kind: MediaKind\", title FROM watchlist_addition_receipts WHERE request_id = ?", request_id)
        .fetch_optional(db).await?)
}

/// Persist a plan and immutable receipt together. Rejected additions consume no retry ID.
pub async fn add_entry(db: &SqlitePool, mut request: AddEntry) -> Result<Addition> {
    request.title = text(&request.title, "title", 500)?;
    request.request_id = text(&request.request_id, "request_id", 128)?;
    // Resolve accepted retries before discovery, so removed owners cannot revive an intention.
    if let Some(receipt) = previous(db, &request.request_id).await? {
        return receipt.replay(db, &request).await;
    }
    let Some(owner_id) = &request.owner_id else {
        let choices = sqlx::query_as!(
            HouseholdUser,
            "SELECT id, name FROM household_users ORDER BY name, id"
        )
        .fetch_all(db)
        .await?;
        return Err(WatchlistError::DestinationRequired { choices });
    };
    if sqlx::query!("SELECT id FROM household_users WHERE id = ?", owner_id)
        .fetch_optional(db)
        .await?
        .is_none()
    {
        return Err(WatchlistError::NotFound(
            "Household User not found; discover household users first".into(),
        ));
    }
    let id = Uuid::new_v4().to_string();
    let date_added = chrono::Utc::now()
        .with_timezone(&chrono_tz::Europe::Copenhagen)
        .date_naive()
        .to_string();
    let mut transaction = db.begin().await?;
    // Claim first to serialize simultaneous retries. The receipt has no cascading foreign key.
    let claimed = sqlx::query!(
        "INSERT INTO watchlist_addition_receipts (request_id, entry_id, owner_id, kind, title) VALUES (?, ?, ?, ?, ?) ON CONFLICT(request_id) DO NOTHING RETURNING request_id",
        request.request_id, id, owner_id, request.kind, request.title)
        .fetch_optional(&mut *transaction).await?;
    if claimed.is_some() {
        let entry = sqlx::query_as!(WatchlistEntry,
            "INSERT INTO watchlist_entries (id, owner_id, kind, title, date_added) VALUES (?, ?, ?, ?, ?) RETURNING id, owner_id, kind AS \"kind: MediaKind\", title, status AS \"status: WatchStatus\", date_added",
            id, owner_id, request.kind, request.title, date_added)
            .fetch_one(&mut *transaction).await?;
        transaction.commit().await?;
        return Ok(Addition {
            entry,
            replayed: false,
        });
    }
    transaction.rollback().await?;
    previous(db, &request.request_id)
        .await?
        .ok_or_else(|| {
            WatchlistError::Conflict(
                "Retry receipt unavailable; refresh before trying again".into(),
            )
        })?
        .replay(db, &request)
        .await
}

/// Remove one plan within its explicit owner. Keep its accepted receipt retired.
pub async fn remove_entry(db: &SqlitePool, owner_id: &str, id: &str) -> Result<()> {
    let removed = sqlx::query!(
        "DELETE FROM watchlist_entries WHERE id = ? AND owner_id = ?",
        id,
        owner_id
    )
    .execute(db)
    .await?;
    if removed.rows_affected() == 0 {
        return Err(WatchlistError::NotFound(
            "Watchlist Entry not found for this Household User".into(),
        ));
    }
    Ok(())
}
