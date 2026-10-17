import { createFileRoute, Link, useRouter } from "@tanstack/react-router"
import { useServerFn } from "@tanstack/react-start"
import { FilmSlateIcon } from "@phosphor-icons/react/dist/csr/FilmSlate"
import { GiftIcon } from "@phosphor-icons/react/dist/csr/Gift"
import { GiftBoard } from "@/components/gift-board"
import { PageHead } from "@/components/page-head"
import { Programme } from "@/components/programme"
import { listsSearch } from "@/lib/lists"
import {
  loadWatchlists,
  mutateWatchlist,
  searchWatchlist,
} from "@/lib/watchlist.functions"
import {
  getWishlistUrlMetadata,
  loadWishlists,
  mutateWishlist,
} from "@/lib/wishlist.functions"

/** Wishes and watching on one page: a Wish | Watch switch and one shared person filter. */
export const Route = createFileRoute("/lists")({
  head: () => ({ meta: [{ title: "Lists · OpenHome" }] }),
  validateSearch: listsSearch,
  loaderDeps: ({ search }) => ({ tab: search.tab }),
  loader: async ({ deps }) =>
    deps.tab === "watch"
      ? { tab: "watch" as const, data: await loadWatchlists() }
      : { tab: "wish" as const, data: await loadWishlists() },
  headers: () => ({ "Cache-Control": "no-store" }),
  component: ListsPage,
})

const tabs = [
  { tab: "wish", label: "Wish", Icon: GiftIcon },
  { tab: "watch", label: "Watch", Icon: FilmSlateIcon },
] as const

function ListsPage(): React.JSX.Element {
  const loaded = Route.useLoaderData()
  const search = Route.useSearch()
  const navigate = Route.useNavigate()
  const router = useRouter()
  const mutateWish = useServerFn(mutateWishlist)
  const preview = useServerFn(getWishlistUrlMetadata)
  const mutateWatch = useServerFn(mutateWatchlist)
  const find = useServerFn(searchWatchlist)
  const refresh = () => router.invalidate({ sync: true })

  const switcher = (
    <nav className="view-tabs" aria-label="Lists">
      {tabs.map((item) => (
        <Link
          key={item.label}
          to="/lists"
          search={(previous) => ({ ...previous, tab: item.tab })}
          activeOptions={{
            exact: true,
            includeSearch: true,
            explicitUndefined: true,
          }}
          className="view-tab"
          aria-current={item.tab === loaded.tab ? "page" : undefined}
          resetScroll={false}
        >
          <item.Icon size={20} weight="bold" aria-hidden="true" />
          {item.label}
        </Link>
      ))}
    </nav>
  )

  return (
    <section>
      <PageHead title="Lists" />
      {loaded.tab === "watch" ? (
        <Programme
          lead={switcher}
          data={loaded.data}
          search={search}
          choose={(next) =>
            void navigate({
              search: (previous) => ({ ...previous, ...next, list: undefined }),
              replace: true,
              resetScroll: false,
            })
          }
          change={(command) => mutateWatch({ data: command })}
          find={(input) => find({ data: input })}
          refresh={refresh}
        />
      ) : (
        <GiftBoard
          lead={switcher}
          data={loaded.data}
          search={search}
          choose={(next) =>
            void navigate({
              search: (previous) => ({ ...previous, ...next }),
              replace: true,
              resetScroll: false,
            })
          }
          change={(command) => mutateWish({ data: command })}
          metadata={(request) => preview({ data: request })}
          refresh={refresh}
        />
      )}
    </section>
  )
}
