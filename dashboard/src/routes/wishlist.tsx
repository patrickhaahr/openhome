import { createFileRoute, useRouter } from "@tanstack/react-router"
import { PageHead } from "@/components/page-head"
import { Wishlists } from "@/components/wishlists"
import {
  loadWishlists,
  mutateWishlist,
  getWishlistUrlMetadata,
} from "@/lib/wishlist.functions"

/** Owner-aware shared Wishlists, backed by the same Axum persistence as MCP. */
export const Route = createFileRoute("/wishlist")({
  head: () => ({ meta: [{ title: "Wishlist · OpenHome" }] }),
  loader: loadWishlists,
  headers: () => ({ "Cache-Control": "no-store" }),
  component: WishlistPage,
})

function WishlistPage(): React.JSX.Element {
  const data = Route.useLoaderData()
  const router = useRouter()

  return (
    <section>
      <PageHead title="Wishlist" />
      <Wishlists
        data={data}
        change={(command) => mutateWishlist({ data: command })}
        metadata={(request) => getWishlistUrlMetadata({ data: request })}
        refresh={() => router.invalidate({ sync: true })}
      />
    </section>
  )
}
