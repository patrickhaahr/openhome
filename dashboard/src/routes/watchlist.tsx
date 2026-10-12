import { createFileRoute, useRouter } from "@tanstack/react-router"
import { useServerFn } from "@tanstack/react-start"
import { watchlistFilters } from "@/lib/watchlist"
import { Watchlists } from "@/components/watchlists"
import { loadWatchlists, mutateWatchlist } from "@/lib/watchlist.functions"

/** Shared household plans, with Axum-owned persistence and explicit-owner writes. */
export const Route = createFileRoute("/watchlist")({
  head: () => ({ meta: [{ title: "Watchlist · OpenHome" }] }),
  validateSearch: watchlistFilters,
  loader: loadWatchlists,
  headers: () => ({ "Cache-Control": "no-store" }),
  component: WatchlistPage,
})

function WatchlistPage(): React.JSX.Element {
  const data = Route.useLoaderData()
  const router = useRouter()
  const filters = Route.useSearch()
  const navigate = Route.useNavigate()
  const mutate = useServerFn(mutateWatchlist)

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
        filters={filters}
        filter={(next) =>
          void navigate({ search: next, replace: true, resetScroll: false })
        }
        change={(command) => mutate({ data: command })}
        refresh={() => router.invalidate({ sync: true })}
      />
    </section>
  )
}
