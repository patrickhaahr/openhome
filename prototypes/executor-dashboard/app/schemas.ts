import { array, object, string } from "apps";

/** A synthetic wishlist entry stored only in the isolated prototype. */
export const WishlistItem = object({ id: string(), owner: string(), title: string() });

/** Shared server/client schema for the prototype's wishlist query. */
export const WishlistItems = array(WishlistItem);
