use rmcp::schemars;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateWishlistRequest {
    /// Stable Household User ID returned by `household_users`.
    pub owner_id: String,
    /// Name of the new Wishlist (1-200 characters, unique for this owner).
    pub name: String,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AddWishlistItemRequest {
    /// Generic desired thing, 1-500 characters. Do not silently choose a product.
    pub title: String,
    /// Unique ID for this intended addition, 1-128 characters. Reuse this ID and
    /// identical arguments after an uncertain response; use a new ID for a new addition.
    pub request_id: String,
    /// Explicit owner ID. Without `list_id` this owner must have exactly one list.
    pub owner_id: Option<String>,
    /// Explicit Wishlist ID. When `owner_id` is supplied, the list must belong to it.
    pub list_id: Option<String>,
    /// Optional product link supplied by the user; no metadata is fetched.
    pub url: Option<String>,
    /// Optional HTTP(S) image link supplied by the user.
    pub image_url: Option<String>,
    /// Supporting details (up to 5000 characters).
    pub notes: Option<String>,
    /// Price text with currency, e.g. DKK 199.95 (up to 100 characters).
    pub price: Option<String>,
    /// Desired count, 1-999; defaults to 1 when absent.
    #[serde(default = "one")]
    pub quantity: i64,
    /// low, normal or high; defaults to normal when absent.
    #[serde(default = "normal")]
    pub priority: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WishlistItemsRequest {
    /// Stable Wishlist ID returned by `wishlists` or `create_wishlist`.
    pub list_id: String,
}

fn one() -> i64 {
    1
}
fn normal() -> String {
    "normal".into()
}

/// Explicit stable destination for reading or deleting one item.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WishlistItemRequest {
    #[serde(rename = "owner_id")]
    pub owner: String,
    #[serde(rename = "list_id")]
    pub list: String,
    #[serde(rename = "item_id")]
    pub item: String,
}

/// Distinguishes omitted fields from explicit null without interpreting their values.
#[derive(Debug, Default, Serialize, schemars::JsonSchema)]
#[serde(untagged)]
pub enum Patch<T> {
    #[default]
    Missing,
    Value(Option<T>),
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Patch<T> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Option::<T>::deserialize(deserializer).map(Self::Value)
    }
}

impl<T> Patch<T> {
    fn missing(&self) -> bool {
        matches!(self, Self::Missing)
    }
}

/// Partial update with Axum-owned validation and nullable-field clearing.
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateWishlistItemRequest {
    #[serde(skip_serializing)]
    pub owner_id: String,
    #[serde(skip_serializing)]
    pub list_id: String,
    #[serde(skip_serializing)]
    pub item_id: String,
    #[serde(default, skip_serializing_if = "Patch::missing")]
    pub title: Patch<String>,
    /// Omit to preserve; null clears the link. Only absolute HTTP(S) links are accepted.
    #[serde(default, skip_serializing_if = "Patch::missing")]
    pub url: Patch<String>,
    /// Omit to preserve; null clears the image link.
    #[serde(default, skip_serializing_if = "Patch::missing")]
    pub image_url: Patch<String>,
    /// Omit to preserve; null clears notes.
    #[serde(default, skip_serializing_if = "Patch::missing")]
    pub notes: Patch<String>,
    /// Price text with currency; omit to preserve, null to clear.
    #[serde(default, skip_serializing_if = "Patch::missing")]
    pub price: Patch<String>,
    /// Desired count, 1-999; cannot be null.
    #[serde(default, skip_serializing_if = "Patch::missing")]
    pub quantity: Patch<i64>,
    /// low, normal or high; cannot be null.
    #[serde(default, skip_serializing_if = "Patch::missing")]
    pub priority: Patch<String>,
}
