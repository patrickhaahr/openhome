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
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WishlistItemsRequest {
    /// Stable Wishlist ID returned by `wishlists` or `create_wishlist`.
    pub list_id: String,
}
