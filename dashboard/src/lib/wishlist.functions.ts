import { createServerFn } from "@tanstack/react-start"
import { setResponseHeader } from "@tanstack/react-start/server"
import {
  wishlistCommand,
  wishlistMetadataRequest,
  type WishlistState,
} from "./wishlist"

export const getWishlistUrlMetadata = createServerFn({ method: "POST" })
  .validator(wishlistMetadataRequest)
  .handler(async ({ data }) => {
    setResponseHeader("Cache-Control", "no-store")
    const { previewWishlistUrl } = await import("./wishlist.server")

    return previewWishlistUrl(data)
  })

export const getWishlists = createServerFn({ method: "GET" }).handler(
  async () => {
    setResponseHeader("Cache-Control", "no-store")
    const { readWishlists } = await import("./wishlist.server")

    return readWishlists()
  }
)

export const mutateWishlist = createServerFn({ method: "POST" })
  .validator(wishlistCommand)
  .handler(async ({ data }) => {
    setResponseHeader("Cache-Control", "no-store")
    const { writeWishlist } = await import("./wishlist.server")

    return writeWishlist(data)
  })

export async function loadWishlists(): Promise<WishlistState> {
  try {
    return await getWishlists()
  } catch {
    return { state: "unavailable", reason: "unreachable" }
  }
}
