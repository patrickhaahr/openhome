import { createFileRoute, useRouter } from "@tanstack/react-router"
import { useServerFn } from "@tanstack/react-start"
import { BinderBook } from "@/components/binder-book"
import { PageHead } from "@/components/page-head"
import { binderSearch } from "@/lib/binder"
import {
  addCard,
  loadBinder,
  loadCardPrices,
  mutateBinder,
  searchCards,
} from "@/lib/binder.functions"

export const Route = createFileRoute("/binder")({
  head: () => ({ meta: [{ title: "Binder · OpenHome" }] }),
  validateSearch: binderSearch,
  loader: () => loadBinder(),
  staleTime: Number.POSITIVE_INFINITY,
  gcTime: 0,
  headers: () => ({ "Cache-Control": "no-store" }),
  component: BinderPage,
  pendingComponent: () => (
    <p role="status" className="inline-flex text-muted-ink">
      Loading Binder…
    </p>
  ),
})

function BinderPage(): React.JSX.Element {
  const data = Route.useLoaderData()
  const search = Route.useSearch()
  const navigate = Route.useNavigate()
  const router = useRouter()
  const mutate = useServerFn(mutateBinder)

  return (
    <section data-slot="binder">
      <PageHead title="Binder" />
      <BinderBook
        data={data}
        search={search}
        choose={(next) =>
          void navigate({ search: next, replace: true, resetScroll: false })
        }
        change={(command) => mutate({ data: command })}
        refresh={() => router.invalidate({ sync: true })}
        find={(input) => searchCards({ data: input })}
        add={(command) => addCard({ data: command })}
        prices={loadCardPrices}
      />
    </section>
  )
}
