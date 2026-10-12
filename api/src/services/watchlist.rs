//! Household-visible movie and whole-series plans, with explicit owners and durable retry identity.

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use uuid::Uuid;

use super::{patch::Patch, tmdb::TmdbService, wishlist::HouseholdUser};

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
    #[error("TMDB is unavailable; you can still add a manual plan")]
    ProviderUnavailable,
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

/// Personal viewing state for a movie or whole series.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, sqlx::Type)]
#[serde(rename_all = "lowercase")]
#[sqlx(rename_all = "lowercase")]
pub enum WatchStatus {
    Planned,
    Watching,
    Completed,
}

#[derive(Debug, Serialize)]
pub struct WatchlistEntry {
    pub id: String,
    pub owner_id: String,
    pub kind: MediaKind,
    pub title: String,
    pub status: WatchStatus,
    pub date_added: String,
    pub completion_date: Option<String>,
    pub tmdb_id: Option<i64>,
    pub overview: Option<String>,
    pub poster_path: Option<String>,
    pub release_date: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AddEntry {
    pub owner_id: Option<String>,
    pub kind: MediaKind,
    pub title: String,
    pub request_id: String,
    pub tmdb_id: Option<i64>,
}

#[derive(Serialize)]
pub struct Addition {
    pub entry: WatchlistEntry,
    pub replayed: bool,
}

struct AdditionReceipt {
    tmdb_id: Option<i64>,
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
            || request.tmdb_id != self.tmdb_id
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
pub async fn entries(db: &SqlitePool, filter: EntriesFilter) -> Result<Vec<WatchlistEntry>> {
    Ok(sqlx::query_as!(WatchlistEntry,
        "SELECT id, owner_id, kind AS \"kind: MediaKind\", title, status AS \"status: WatchStatus\", date_added, completion_date, tmdb_id, overview, poster_path, release_date FROM watchlist_entries WHERE (? IS NULL OR owner_id = ?) AND (? IS NULL OR status = ?) ORDER BY date_added DESC, id", filter.owner_id, filter.owner_id, filter.status, filter.status)
        .fetch_all(db).await?)
}

/// Read only the complete owner/entry destination.
pub async fn entry(db: &SqlitePool, owner_id: &str, id: &str) -> Result<WatchlistEntry> {
    sqlx::query_as!(WatchlistEntry,
        "SELECT id, owner_id, kind AS \"kind: MediaKind\", title, status AS \"status: WatchStatus\", date_added, completion_date, tmdb_id, overview, poster_path, release_date FROM watchlist_entries WHERE id = ? AND owner_id = ?", id, owner_id)
        .fetch_optional(db).await?
        .ok_or_else(|| WatchlistError::NotFound("Watchlist Entry not found for this Household User".into()))
}

async fn previous(db: &SqlitePool, request_id: &str) -> Result<Option<AdditionReceipt>> {
    Ok(sqlx::query_as!(AdditionReceipt,
        "SELECT tmdb_id, entry_id, owner_id, kind AS \"kind: MediaKind\", title FROM watchlist_addition_receipts WHERE request_id = ?", request_id)
        .fetch_optional(db).await?)
}

/// Persist a plan and immutable receipt together. Rejected additions consume no retry ID.
pub async fn add_entry(
    db: &SqlitePool,
    mut request: AddEntry,
    provider: Option<&TmdbService>,
) -> Result<Addition> {
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
    let metadata = match request.tmdb_id {
        Some(id) => Some(
            provider
                .ok_or(WatchlistError::ProviderUnavailable)?
                .details(request.kind, id)
                .await?,
        ),
        None => None,
    };
    let stored_title = metadata
        .as_ref()
        .map_or(request.title.as_str(), |value| value.title.as_str());
    let overview = metadata
        .as_ref()
        .and_then(|value| value.overview.as_deref());
    let poster_path = metadata
        .as_ref()
        .and_then(|value| value.poster_path.as_deref());
    let release_date = metadata
        .as_ref()
        .and_then(|value| value.release_date.as_deref());
    let id = Uuid::new_v4().to_string();
    let date_added = today();
    let mut transaction = db.begin().await?;
    // Claim first to serialize simultaneous retries. The receipt has no cascading foreign key.
    let claimed = sqlx::query!(
        "INSERT INTO watchlist_addition_receipts (request_id, entry_id, owner_id, kind, title, tmdb_id) VALUES (?, ?, ?, ?, ?, ?) ON CONFLICT(request_id) DO NOTHING RETURNING request_id",
        request.request_id, id, owner_id, request.kind, request.title, request.tmdb_id)
        .fetch_optional(&mut *transaction).await?;
    if claimed.is_some() {
        let entry = sqlx::query_as!(WatchlistEntry,
            "INSERT INTO watchlist_entries (id, owner_id, kind, title, date_added, tmdb_id, overview, poster_path, release_date) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING id, owner_id, kind AS \"kind: MediaKind\", title, status AS \"status: WatchStatus\", date_added, completion_date, tmdb_id, overview, poster_path, release_date",
            id, owner_id, request.kind, stored_title, date_added, request.tmdb_id, overview, poster_path, release_date)
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

/// Optional owner and status filters; omission retains household-wide visibility.
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntriesFilter {
    pub owner_id: Option<String>,
    pub status: Option<WatchStatus>,
}

/// Omitted fields preserve values; null only clears a non-completed entry's completion date.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EditEntry {
    #[serde(default)]
    pub status: Patch<WatchStatus>,
    #[serde(default)]
    pub date_added: Patch<String>,
    #[serde(default)]
    pub completion_date: Patch<String>,
}

fn today() -> String {
    chrono::Utc::now()
        .with_timezone(&chrono_tz::Europe::Copenhagen)
        .date_naive()
        .to_string()
}

fn calendar_date(value: String, field: &str) -> Result<String> {
    if value.len() != 10
        || value.starts_with("0000")
        || chrono::NaiveDate::parse_from_str(&value, "%Y-%m-%d").is_err()
        || !value.bytes().enumerate().all(|(i, c)| {
            if i == 4 || i == 7 {
                c == b'-'
            } else {
                c.is_ascii_digit()
            }
        })
    {
        return Err(WatchlistError::Validation(format!(
            "{field} must be a real calendar date in YYYY-MM-DD (years 0001-9999)"
        )));
    }
    Ok(value)
}

/// Apply one validated correction to an explicit destination, rejecting concurrent changes.
pub async fn edit_entry(
    db: &SqlitePool,
    owner_id: &str,
    id: &str,
    request: EditEntry,
) -> Result<WatchlistEntry> {
    let current = entry(db, owner_id, id).await?;
    let status = match request.status {
        Patch::Missing => current.status,
        Patch::Value(Some(value)) => value,
        Patch::Value(None) => {
            return Err(WatchlistError::Validation("status cannot be null".into()));
        }
    };
    let date_added = match request.date_added {
        Patch::Missing => current.date_added.clone(),
        Patch::Value(Some(value)) => calendar_date(value, "date_added")?,
        Patch::Value(None) => {
            return Err(WatchlistError::Validation(
                "date_added cannot be null".into(),
            ));
        }
    };
    let completion_date = match (status, request.completion_date) {
        (WatchStatus::Completed, Patch::Missing) => {
            Some(current.completion_date.clone().unwrap_or_else(today))
        }
        (WatchStatus::Completed, Patch::Value(Some(value))) => {
            Some(calendar_date(value, "completion_date")?)
        }
        (WatchStatus::Completed, Patch::Value(None)) => {
            return Err(WatchlistError::Validation(
                "Completed entries require a completion_date".into(),
            ));
        }
        (_, Patch::Value(Some(_))) => {
            return Err(WatchlistError::Validation(
                "Only completed entries can have a completion_date".into(),
            ));
        }
        (_, _) => None,
    };
    if completion_date
        .as_ref()
        .is_some_and(|completed| completed < &date_added)
    {
        return Err(WatchlistError::Validation(
            "completion_date cannot precede date_added; correct the dates together".into(),
        ));
    }
    // Compare the snapshot in the same statement as the write, avoiding lost corrections.
    sqlx::query_as!(WatchlistEntry,
        "UPDATE watchlist_entries SET status = ?, date_added = ?, completion_date = ? WHERE id = ? AND owner_id = ? AND status = ? AND date_added = ? AND completion_date IS ? RETURNING id, owner_id, kind AS \"kind: MediaKind\", title, status AS \"status: WatchStatus\", date_added, completion_date, tmdb_id, overview, poster_path, release_date",
        status, date_added, completion_date, id, owner_id, current.status, current.date_added, current.completion_date)
        .fetch_optional(db).await?
        .ok_or_else(|| WatchlistError::Conflict("Entry changed during this correction; refresh before retrying".into()))
}

/// Enrich only metadata at an explicit destination; preserve all personal status/date fields.
pub async fn enrich_entry(
    db: &SqlitePool,
    owner_id: &str,
    id: &str,
    tmdb_id: i64,
    provider: Option<&TmdbService>,
) -> Result<WatchlistEntry> {
    let current = entry(db, owner_id, id).await?;
    let metadata = provider
        .ok_or(WatchlistError::ProviderUnavailable)?
        .details(current.kind, tmdb_id)
        .await?;
    sqlx::query_as!(WatchlistEntry,
        "UPDATE watchlist_entries SET tmdb_id = ?, title = ?, overview = ?, poster_path = ?, release_date = ? WHERE id = ? AND owner_id = ? AND tmdb_id IS ? RETURNING id, owner_id, kind AS \"kind: MediaKind\", title, status AS \"status: WatchStatus\", date_added, completion_date, tmdb_id, overview, poster_path, release_date",
        tmdb_id, metadata.title, metadata.overview, metadata.poster_path, metadata.release_date, id, owner_id, current.tmdb_id)
        .fetch_optional(db).await?
        .ok_or_else(|| WatchlistError::Conflict("Entry metadata changed; refresh before retrying".into()))
}
