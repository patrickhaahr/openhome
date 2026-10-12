import { createServerFn } from "@tanstack/react-start"
import { setResponseHeader } from "@tanstack/react-start/server"
import {
  wishlistCommand,
  wishlistMetadataRequest,
  type WishlistState,
} from "./wishlist"

/** Non-persistent URL assistance through the same authenticated Axum operation as MCP. */
export const getWishlistUrlMetadata = createServerFn({ method: "POST" })
  .validator(wishlistMetadataRequest)
  .handler(async ({ data }) => {
    setResponseHeader("Cache-Control", "no-store")
    const { previewWishlistUrl } = await import("./wishlist.server")

    return previewWishlistUrl(data)
  })

/** Read shared Wishlists through the server-held Axum credential. */
export const getWishlists = createServerFn({ method: "GET" }).handler(
  async () => {
    setResponseHeader("Cache-Control", "no-store")
    const { readWishlists } = await import("./wishlist.server")

    return readWishlists()
  }
)

/** Persist a destination-explicit change through Axum, keeping credentials server-side. */
export const mutateWishlist = createServerFn({ method: "POST" })
  .validator(wishlistCommand)
  .handler(async ({ data }) => {
    setResponseHeader("Cache-Control", "no-store")
    const { writeWishlist } = await import("./wishlist.server")

    return writeWishlist(data)
  })

/** A dropped read RPC renders recovery controls instead of a route error. */
export async function loadWishlists(): Promise<WishlistState> {
  try {
    return await getWishlists()
  } catch {
    return { state: "unavailable", reason: "unreachable" }
  }
}
