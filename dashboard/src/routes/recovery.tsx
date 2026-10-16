import { createFileRoute, useRouter } from "@tanstack/react-router"
import { PageHead } from "@/components/page-head"
import { TrainingSummaries } from "@/components/training-summaries"
import { trainingSearch } from "@/lib/training"
import { getRecoveryDays, loadTraining } from "@/lib/training.functions"

/** Recovery: the selected day's rings over its week, its night, its body and its strap sessions. */
export const Route = createFileRoute("/recovery")({
  head: () => ({ meta: [{ title: "Recovery · OpenHome" }] }),
  validateSearch: trainingSearch,
  loaderDeps: ({ search }) => ({ day: search.day, noop: search.noop }),
  loader: ({ deps }) => loadTraining(deps),
  staleTime: 0,
  headers: () => ({ "Cache-Control": "no-store" }),
  component: RecoveryPage,
})

function RecoveryPage(): React.JSX.Element {
  const data = Route.useLoaderData()
  const search = Route.useSearch()
  const navigate = Route.useNavigate()
  const router = useRouter()

  return (
    <>
      <PageHead title="Recovery" />
      <TrainingSummaries
        key={data.day}
        data={data}
        loadDays={(from, to) => getRecoveryDays({ data: { from, to } })}
        refresh={() => router.invalidate({ sync: true })}
        selectDay={async (day) => {
          if (day === data.day && search.noop === undefined)
            await router.invalidate({ sync: true })
          else
            await navigate({
              search: (previous) => ({ ...previous, day, noop: undefined }),
              resetScroll: false,
            })
        }}
        open={async (noop) => {
          if (
            noop?.source === search.noop?.source &&
            noop?.start === search.noop?.start &&
            noop?.sport === search.noop?.sport
          )
            await router.invalidate({ sync: true })
          else
            await navigate({
              search: (previous) => ({ ...previous, noop }),
              resetScroll: false,
            })
        }}
      />
    </>
  )
}
