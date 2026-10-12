import { createFileRoute, useRouter } from "@tanstack/react-router"
import { Watchlists } from "@/components/watchlists"
import { loadWatchlists, mutateWatchlist } from "@/lib/watchlist.functions"

/** Shared household plans, with Axum-owned persistence and explicit-owner writes. */
export const Route = createFileRoute("/watchlist")({
  head: () => ({ meta: [{ title: "Watchlist · OpenHome" }] }),
  loader: loadWatchlists,
  headers: () => ({ "Cache-Control": "no-store" }),
  component: WatchlistPage,
})

function WatchlistPage(): React.JSX.Element {
  const data = Route.useLoaderData()
  const router = useRouter()

  return (
    <section className="focused-page watchlist-page">
      <header>
        <p className="page-eyebrow">OpenHome / Watchlist</p>
        <h1 id="page-heading" tabIndex={-1}>
          Watchlist
        </h1>
        <p className="feature-intro">
          A good story for another evening. Everyone’s plans, together.
        </p>
      </header>
      <Watchlists
        data={data}
        change={(command) => mutateWatchlist({ data: command })}
        refresh={() => router.invalidate({ sync: true })}
      />
    </section>
  )
}
