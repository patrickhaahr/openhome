//! Household-visible Wishlists. The API resolves destinations and owns retry identity.

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use uuid::Uuid;

use super::patch::Patch;

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
    pub url: Option<String>,
    pub image_url: Option<String>,
    pub notes: Option<String>,
    pub price: Option<String>,
    pub quantity: i64,
    pub priority: String,
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
    pub url: Option<String>,
    pub image_url: Option<String>,
    pub notes: Option<String>,
    pub price: Option<String>,
    #[serde(default = "one")]
    pub quantity: i64,
    #[serde(default = "normal")]
    pub priority: String,
}

#[derive(Serialize)]
pub struct Addition {
    pub item: WishlistItem,
    pub replayed: bool,
}

struct AdditionReceipt {
    item_id: String,
    title: String,
    requested_owner_id: Option<String>,
    requested_list_id: Option<String>,
    details: String,
}

impl AdditionReceipt {
    async fn replay(self, db: &SqlitePool, request: &AddItem) -> Result<Addition> {
        if self.title != request.title
            || self.requested_owner_id != request.owner_id
            || self.requested_list_id != request.list_id
            || self.details != request.details()
        {
            return Err(WishlistError::Conflict(
                "request_id was already used for a different addition; use a new ID for a new request"
                    .into(),
            ));
        }
        let item = sqlx::query_as!(
            WishlistItem,
            "SELECT id, list_id, title, created_at, url, image_url, notes, price, quantity, priority FROM wishlist_items WHERE id = ?",
            self.item_id
        )
        .fetch_optional(db)
        .await?
        .ok_or_else(|| {
            WishlistError::Conflict(
                "The retried item was removed; use a new request_id to add it again".into(),
            )
        })?;
        Ok(Addition {
            item,
            replayed: true,
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

/// Require an explicit owner/list pair before metadata assistance or item writes.
pub async fn require_list_owner(db: &SqlitePool, owner_id: &str, list_id: &str) -> Result<()> {
    let list = get_list(db, list_id).await?;
    if list.owner_id != owner_id {
        return Err(WishlistError::Conflict(
            "Wishlist does not belong to this Household User".into(),
        ));
    }
    Ok(())
}

pub async fn items(db: &SqlitePool, list_id: &str) -> Result<Vec<WishlistItem>> {
    get_list(db, list_id).await?;
    Ok(sqlx::query_as!(WishlistItem,
        "SELECT id, list_id, title, created_at, url, image_url, notes, price, quantity, priority FROM wishlist_items WHERE list_id = ? ORDER BY created_at, id", list_id)
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

async fn previous(db: &SqlitePool, request_id: &str) -> Result<Option<AdditionReceipt>> {
    Ok(sqlx::query_as!(AdditionReceipt,
        "SELECT item_id, title, requested_owner_id, requested_list_id, details FROM wishlist_addition_receipts WHERE request_id = ?", request_id)
        .fetch_optional(db).await?)
}

pub async fn add_item(
    db: &SqlitePool,
    mut request: AddItem,
    default_list_id: Option<&str>,
) -> Result<Addition> {
    request.title = text(&request.title, "title", 500)?;
    request.request_id = text(&request.request_id, "request_id", 128)?;
    request.url = optional_url(request.url)?;
    request.image_url = optional_url(request.image_url)?;
    request.notes = notes(request.notes)?;
    request.price = optional_text(request.price, "price", 100)?;
    quantity(request.quantity)?;
    priority(&request.priority)?;
    let details = request.details();
    // Replay before resolving: a changed default or newly created list cannot move an accepted request.
    if let Some(item) = previous(db, &request.request_id).await? {
        return item.replay(db, &request).await;
    }
    let list = destination(db, &request, default_list_id).await?;
    let id = Uuid::new_v4().to_string();
    let mut transaction = db.begin().await?;
    // Claim before inserting, in one transaction. Receipts have no foreign key so delayed
    // retries cannot recreate a deleted item, list or owner. Failed writes consume no ID.
    let claimed = sqlx::query!(
        "INSERT INTO wishlist_addition_receipts (request_id, item_id, title, requested_owner_id, requested_list_id, details) VALUES (?, ?, ?, ?, ?, ?) ON CONFLICT(request_id) DO NOTHING RETURNING request_id",
        request.request_id, id, request.title, request.owner_id, request.list_id, details)
        .fetch_optional(&mut *transaction).await?;
    if claimed.is_some() {
        let item = sqlx::query_as!(WishlistItem,
            "INSERT INTO wishlist_items (id, list_id, title, request_id, requested_owner_id, requested_list_id, url, image_url, notes, price, quantity, priority) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING id, list_id, title, created_at, url, image_url, notes, price, quantity, priority",
            id, list.id, request.title, request.request_id, request.owner_id, request.list_id, request.url, request.image_url, request.notes, request.price, request.quantity, request.priority)
            .fetch_one(&mut *transaction).await?;
        transaction.commit().await?;
        return Ok(Addition {
            item,
            replayed: false,
        });
    }
    transaction.rollback().await?;
    previous(db, &request.request_id)
        .await?
        .ok_or_else(|| {
            WishlistError::Conflict(
                "The retried item was removed; use a new request_id to add it again".into(),
            )
        })?
        .replay(db, &request)
        .await
}

/// Rename-only changes preserve stable IDs and ownership.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rename {
    pub name: String,
}

/// Partial item edits preserve the original addition's retry identity and destination.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EditItem {
    #[serde(default)]
    pub title: Patch<String>,
    #[serde(default)]
    pub url: Patch<String>,
    #[serde(default)]
    pub image_url: Patch<String>,
    #[serde(default)]
    pub notes: Patch<String>,
    #[serde(default)]
    pub price: Patch<String>,
    #[serde(default)]
    pub quantity: Patch<i64>,
    #[serde(default)]
    pub priority: Patch<String>,
}

fn one() -> i64 {
    1
}
fn normal() -> String {
    "normal".into()
}

fn quantity(value: i64) -> Result<()> {
    if !(1..=999).contains(&value) {
        return Err(WishlistError::Validation(
            "quantity must be an integer from 1 to 999".into(),
        ));
    }
    Ok(())
}

fn priority(value: &str) -> Result<()> {
    if !matches!(value, "low" | "normal" | "high") {
        return Err(WishlistError::Validation(
            "priority must be low, normal or high".into(),
        ));
    }
    Ok(())
}

fn optional_text(value: Option<String>, field: &str, max: usize) -> Result<Option<String>> {
    value.map(|value| text(&value, field, max)).transpose()
}

fn notes(value: Option<String>) -> Result<Option<String>> {
    value.map(|value| {
        let value = value.trim();
        if value.is_empty() || value.chars().count() > 5000 || value.chars().any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t')) {
            return Err(WishlistError::Validation("notes must contain 1-5000 characters; only line breaks and tabs are allowed as controls".into()));
        }
        Ok(value.to_owned())
    }).transpose()
}

fn optional_url(value: Option<String>) -> Result<Option<String>> {
    let value = optional_text(value, "URL", 2048)?;
    if let Some(value) = &value {
        let url = reqwest::Url::parse(value).map_err(|_| {
            WishlistError::Validation("URL must be an absolute HTTP(S) link".into())
        })?;
        if !matches!(url.scheme(), "http" | "https")
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
        {
            return Err(WishlistError::Validation(
                "URL must be an absolute HTTP(S) link without credentials".into(),
            ));
        }
    }
    Ok(value)
}

impl AddItem {
    fn details(&self) -> String {
        // Fixed JSON keys also backfill legacy title-only receipts in migration 0008.
        serde_json::json!({"url":self.url,"image_url":self.image_url,"notes":self.notes,
            "price":self.price,"quantity":self.quantity,"priority":self.priority})
        .to_string()
    }
}

/// Rename one configured household member.
pub async fn rename_owner(db: &SqlitePool, id: &str, request: Rename) -> Result<HouseholdUser> {
    let name = text(&request.name, "name", 200)?;
    sqlx::query_as!(
        HouseholdUser,
        "UPDATE household_users SET name = ? WHERE id = ? RETURNING id, name",
        name,
        id
    )
    .fetch_optional(db)
    .await?
    .ok_or_else(|| WishlistError::NotFound("Household User not found".into()))
}

/// Delete a member and cascade all their lists/items. Addition receipts remain retired.
pub async fn remove_owner(db: &SqlitePool, id: &str) -> Result<()> {
    let removed = sqlx::query!("DELETE FROM household_users WHERE id = ?", id)
        .execute(db)
        .await?;
    changed(removed.rows_affected())
}

/// Rename a list only within its explicit owner.
pub async fn rename_list(
    db: &SqlitePool,
    owner_id: &str,
    id: &str,
    request: Rename,
) -> Result<Wishlist> {
    let name = text(&request.name, "name", 200)?;
    sqlx::query_as!(
        Wishlist,
        "UPDATE wishlists SET name = ? WHERE id = ? AND owner_id = ? RETURNING id, owner_id, name",
        name,
        id,
        owner_id
    )
    .fetch_optional(db)
    .await?
    .ok_or_else(|| WishlistError::NotFound("Wishlist not found for this Household User".into()))
}

/// Remove a list and its items only within its explicit owner.
pub async fn remove_list(db: &SqlitePool, owner_id: &str, id: &str) -> Result<()> {
    let removed = sqlx::query!(
        "DELETE FROM wishlists WHERE id = ? AND owner_id = ?",
        id,
        owner_id
    )
    .execute(db)
    .await?;
    changed(removed.rows_affected())
}

/// Read one item through its complete owner/list/item destination.
pub async fn item(
    db: &SqlitePool,
    owner_id: &str,
    list_id: &str,
    id: &str,
) -> Result<WishlistItem> {
    sqlx::query_as!(WishlistItem,
        "SELECT id, list_id, title, created_at, url, image_url, notes, price, quantity, priority FROM wishlist_items WHERE id = ? AND list_id = ? AND EXISTS (SELECT 1 FROM wishlists WHERE id = ? AND owner_id = ?)", id, list_id, list_id, owner_id)
        .fetch_optional(db).await?
        .ok_or_else(|| WishlistError::NotFound("Wishlist Item not found for this owner and list".into()))
}

/// Atomically patch supplied fields, without moving an item or changing its retry receipt.
pub async fn edit_item(
    db: &SqlitePool,
    owner_id: &str,
    list_id: &str,
    id: &str,
    request: EditItem,
) -> Result<WishlistItem> {
    let title_set = request.title.supplied();
    let url_set = request.url.supplied();
    let image_set = request.image_url.supplied();
    let notes_set = request.notes.supplied();
    let price_set = request.price.supplied();
    let quantity_set = request.quantity.supplied();
    let priority_set = request.priority.supplied();
    let title = optional_text(request.title.value(), "title", 500)?;
    let url = optional_url(request.url.value())?;
    let image_url = optional_url(request.image_url.value())?;
    let notes = notes(request.notes.value())?;
    let price = optional_text(request.price.value(), "price", 100)?;
    let quantity_value = request.quantity.value();
    let priority_value = request.priority.value();
    if (title_set && title.is_none())
        || (quantity_set && quantity_value.is_none())
        || (priority_set && priority_value.is_none())
    {
        return Err(WishlistError::Validation(
            "title, quantity and priority cannot be null".into(),
        ));
    }
    if let Some(value) = quantity_value {
        quantity(value)?;
    }
    if let Some(value) = &priority_value {
        priority(value)?;
    }
    sqlx::query_as!(WishlistItem,
        "UPDATE wishlist_items SET title = CASE WHEN ? THEN ? ELSE title END,
        url = CASE WHEN ? THEN ? ELSE url END, image_url = CASE WHEN ? THEN ? ELSE image_url END,
        notes = CASE WHEN ? THEN ? ELSE notes END, price = CASE WHEN ? THEN ? ELSE price END,
        quantity = CASE WHEN ? THEN ? ELSE quantity END, priority = CASE WHEN ? THEN ? ELSE priority END
        WHERE id = ? AND list_id = ? AND EXISTS (SELECT 1 FROM wishlists WHERE id = ? AND owner_id = ?)
        RETURNING id, list_id, title, created_at, url, image_url, notes, price, quantity, priority",
        title_set, title, url_set, url, image_set, image_url, notes_set, notes, price_set, price,
        quantity_set, quantity_value, priority_set, priority_value, id, list_id, list_id, owner_id)
        .fetch_optional(db).await?
        .ok_or_else(|| WishlistError::NotFound("Wishlist Item not found for this owner and list".into()))
}

/// Remove a title through the complete owner/list/item destination.
pub async fn remove_item(db: &SqlitePool, owner_id: &str, list_id: &str, id: &str) -> Result<()> {
    let removed = sqlx::query!(
        "DELETE FROM wishlist_items WHERE id = ? AND list_id = ? AND EXISTS (SELECT 1 FROM wishlists WHERE id = ? AND owner_id = ?)", id, list_id, list_id, owner_id).execute(db).await?;
    changed(removed.rows_affected())
}

fn changed(rows: u64) -> Result<()> {
    if rows == 0 {
        Err(WishlistError::NotFound(
            "Wishlist destination not found; refresh before trying again".into(),
        ))
    } else {
        Ok(())
    }
}
