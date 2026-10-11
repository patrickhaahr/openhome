import { createFileRoute, useRouter } from "@tanstack/react-router"
import { Wishlists } from "@/components/wishlists"
import { loadWishlists, mutateWishlist } from "@/lib/wishlist.functions"

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
    <section className="focused-page wishlist-page">
      <header>
        <p className="page-eyebrow">OpenHome / Wishlist</p>
        <h1 id="page-heading" tabIndex={-1}>
          Wishlist
        </h1>
        <p className="feature-intro">
          Everyone’s ideas. A shared place to keep the things you’d like.
        </p>
      </header>
      <Wishlists
        data={data}
        change={(command) => mutateWishlist({ data: command })}
        refresh={() => router.invalidate({ sync: true })}
      />
    </section>
  )
}
