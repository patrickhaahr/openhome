//! Household-visible Wishlists. The API resolves destinations and owns retry identity.

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum WishlistError {
    #[error("{0}")]
    Validation(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Conflict(String),
    #[error("Which household owner's wishlist should receive this item?")]
    DestinationRequired { choices: Vec<Wishlist> },
    #[error("Wishlist database operation failed")]
    Database(#[from] sqlx::Error),
}

type Result<T> = std::result::Result<T, WishlistError>;

#[derive(Debug, Serialize)]
pub struct HouseholdUser {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct Wishlist {
    pub id: String,
    pub owner_id: String,
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct WishlistItem {
    pub id: String,
    pub list_id: String,
    pub title: String,
    pub created_at: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateHouseholdUser {
    pub name: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateWishlist {
    pub owner_id: String,
    pub name: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AddItem {
    pub title: String,
    pub request_id: String,
    pub owner_id: Option<String>,
    pub list_id: Option<String>,
}

#[derive(Serialize)]
pub struct Addition {
    pub item: WishlistItem,
    pub replayed: bool,
}

struct StoredItem {
    id: String,
    list_id: String,
    title: String,
    created_at: String,
    requested_owner_id: Option<String>,
    requested_list_id: Option<String>,
}

impl StoredItem {
    fn addition(self, request: &AddItem, replayed: bool) -> Result<Addition> {
        if self.title != request.title
            || self.requested_owner_id != request.owner_id
            || self.requested_list_id != request.list_id
        {
            return Err(WishlistError::Conflict(
                "request_id was already used for a different addition; use a new ID for a new request"
                    .into(),
            ));
        }
        Ok(Addition {
            item: WishlistItem {
                id: self.id,
                list_id: self.list_id,
                title: self.title,
                created_at: self.created_at,
            },
            replayed,
        })
    }
}

fn text(value: &str, field: &str, max: usize) -> Result<String> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > max || value.chars().any(char::is_control) {
        return Err(WishlistError::Validation(format!(
            "{field} must contain 1-{max} characters without control characters"
        )));
    }
    Ok(value.to_owned())
}

pub async fn owners(db: &SqlitePool) -> Result<Vec<HouseholdUser>> {
    Ok(sqlx::query_as!(
        HouseholdUser,
        "SELECT id, name FROM household_users ORDER BY name, id"
    )
    .fetch_all(db)
    .await?)
}

pub async fn create_owner(db: &SqlitePool, request: CreateHouseholdUser) -> Result<HouseholdUser> {
    let name = text(&request.name, "name", 200)?;
    let id = Uuid::new_v4().to_string();
    Ok(sqlx::query_as!(
        HouseholdUser,
        "INSERT INTO household_users (id, name) VALUES (?, ?) RETURNING id, name",
        id,
        name
    )
    .fetch_one(db)
    .await?)
}

async fn require_owner(db: &SqlitePool, owner_id: &str) -> Result<()> {
    if sqlx::query!("SELECT id FROM household_users WHERE id = ?", owner_id)
        .fetch_optional(db)
        .await?
        .is_none()
    {
        return Err(WishlistError::NotFound(
            "Household User not found; discover household users first".into(),
        ));
    }
    Ok(())
}

pub async fn lists(db: &SqlitePool) -> Result<Vec<Wishlist>> {
    Ok(sqlx::query_as!(
        Wishlist,
        "SELECT id, owner_id, name FROM wishlists ORDER BY owner_id, name, id"
    )
    .fetch_all(db)
    .await?)
}

pub async fn create_list(db: &SqlitePool, request: CreateWishlist) -> Result<Wishlist> {
    let name = text(&request.name, "name", 200)?;
    require_owner(db, &request.owner_id).await?;
    let id = Uuid::new_v4().to_string();
    Ok(sqlx::query_as!(
        Wishlist,
        "INSERT INTO wishlists (id, owner_id, name) VALUES (?, ?, ?) RETURNING id, owner_id, name",
        id,
        request.owner_id,
        name
    )
    .fetch_one(db)
    .await?)
}

async fn get_list(db: &SqlitePool, id: &str) -> Result<Wishlist> {
    sqlx::query_as!(
        Wishlist,
        "SELECT id, owner_id, name FROM wishlists WHERE id = ?",
        id
    )
    .fetch_optional(db)
    .await?
    .ok_or_else(|| WishlistError::NotFound("Wishlist not found; discover wishlists first".into()))
}

pub async fn items(db: &SqlitePool, list_id: &str) -> Result<Vec<WishlistItem>> {
    get_list(db, list_id).await?;
    Ok(sqlx::query_as!(WishlistItem,
        "SELECT id, list_id, title, created_at FROM wishlist_items WHERE list_id = ? ORDER BY created_at, id", list_id)
        .fetch_all(db)
        .await?)
}

async fn destination(
    db: &SqlitePool,
    request: &AddItem,
    default_list_id: Option<&str>,
) -> Result<Wishlist> {
    if let Some(list_id) = request.list_id.as_deref() {
        let list = get_list(db, list_id).await?;
        if request
            .owner_id
            .as_ref()
            .is_some_and(|owner| *owner != list.owner_id)
        {
            return Err(WishlistError::Conflict(
                "The selected Wishlist does not belong to the selected Household User".into(),
            ));
        }
        return Ok(list);
    }
    if let Some(owner) = request.owner_id.as_deref() {
        require_owner(db, owner).await?;
        let mut choices = sqlx::query_as!(
            Wishlist,
            "SELECT id, owner_id, name FROM wishlists WHERE owner_id = ? ORDER BY name, id",
            owner
        )
        .fetch_all(db)
        .await?;
        if choices.len() == 1 {
            return Ok(choices.remove(0));
        }
        return Err(WishlistError::DestinationRequired { choices });
    }
    if let Some(list_id) = default_list_id {
        return get_list(db, list_id).await;
    }
    Err(WishlistError::DestinationRequired {
        choices: lists(db).await?,
    })
}

async fn previous(db: &SqlitePool, request_id: &str) -> Result<Option<StoredItem>> {
    Ok(sqlx::query_as!(StoredItem,
        "SELECT id, list_id, title, created_at, requested_owner_id, requested_list_id FROM wishlist_items WHERE request_id = ?", request_id)
        .fetch_optional(db).await?)
}

pub async fn add_item(
    db: &SqlitePool,
    mut request: AddItem,
    default_list_id: Option<&str>,
) -> Result<Addition> {
    request.title = text(&request.title, "title", 500)?;
    request.request_id = text(&request.request_id, "request_id", 128)?;
    // Replay before resolving: a changed default or newly created list cannot move an accepted request.
    if let Some(item) = previous(db, &request.request_id).await? {
        return item.addition(&request, true);
    }
    let list = destination(db, &request, default_list_id).await?;
    let id = Uuid::new_v4().to_string();
    // One atomic statement claims the request ID and creates the item. Concurrent retries cannot
    // both insert; a losing request reads and checks the winning request's original inputs.
    let inserted = sqlx::query_as!(StoredItem,
        "INSERT INTO wishlist_items (id, list_id, title, request_id, requested_owner_id, requested_list_id) VALUES (?, ?, ?, ?, ?, ?) ON CONFLICT(request_id) DO NOTHING RETURNING id, list_id, title, created_at, requested_owner_id, requested_list_id",
        id, list.id, request.title, request.request_id, request.owner_id, request.list_id)
        .fetch_optional(db).await?;
    if let Some(item) = inserted {
        return item.addition(&request, false);
    }
    previous(db, &request.request_id)
        .await?
        .ok_or_else(|| {
            WishlistError::Conflict(
                "The retried item was removed; use a new request_id to add it again".into(),
            )
        })?
        .addition(&request, true)
}
