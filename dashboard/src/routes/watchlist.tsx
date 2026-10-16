import { createFileRoute, useRouter } from "@tanstack/react-router"
import { useServerFn } from "@tanstack/react-start"
import { watchlistFilters } from "@/lib/watchlist"
import { PageHead } from "@/components/page-head"
import { Watchlists } from "@/components/watchlists"
import {
  loadWatchlists,
  mutateWatchlist,
  searchWatchlist,
} from "@/lib/watchlist.functions"

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
  const search = useServerFn(searchWatchlist)

  return (
    <section>
      <PageHead title="Watchlist" />
      <Watchlists
        data={data}
        search={(input) => search({ data: input })}
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
