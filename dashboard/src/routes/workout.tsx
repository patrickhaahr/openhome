import {
  createFileRoute,
  Link,
  redirect,
  useRouter,
} from "@tanstack/react-router"
import { useServerFn } from "@tanstack/react-start"
import { BarbellIcon } from "@phosphor-icons/react/dist/csr/Barbell"
import { ChartLineUpIcon } from "@phosphor-icons/react/dist/csr/ChartLineUp"
import { ScalesIcon } from "@phosphor-icons/react/dist/csr/Scales"
import { z } from "zod"
import { BodyProfile } from "@/components/body-profile"
import { PageHead } from "@/components/page-head"
import { ExerciseProgress } from "@/components/exercise-progress"
import { Workouts } from "@/components/workouts"
import { trainingSearch } from "@/lib/training"
import { bodyRange } from "@/lib/body-profile"
import {
  loadBodyProfile,
  mutateBodyProfile,
} from "@/lib/body-profile.functions"
import { progressSearch } from "@/lib/exercise-progress"
import { loadProgress } from "@/lib/exercise-progress.functions"
import { historyStep, maxHistory, workoutSearch } from "@/lib/workout"
import { loadWorkouts } from "@/lib/workout.functions"

/** Workout views: the log first, then exercise progress and body weight. Recovery has its own page. */
export const Route = createFileRoute("/workout")({
  head: () => ({ meta: [{ title: "Workout · OpenHome" }] }),
  validateSearch: workoutSearch.extend({
    // "today" is the retired recovery view; it only redirects.
    view: z
      .enum(["log", "progress", "body", "today"])
      .optional()
      .catch(undefined),
    ...bodyRange.shape,
    ...progressSearch.shape,
    ...trainingSearch.shape,
  }),
  // Old links to the recovery view, or to a day on it, keep their day and selection.
  beforeLoad: ({ search }) => {
    if (
      search.view === "today" ||
      (search.view === undefined &&
        (search.day !== undefined || search.noop !== undefined))
    )
      throw redirect({
        to: "/recovery",
        search: { day: search.day, noop: search.noop },
        replace: true,
      })
  },
  loaderDeps: ({ search }) => ({
    view: search.view,
    from: search.from,
    to: search.to,
    shown: search.shown,
    period: search.period,
    compare: search.compare,
  }),
  loader: async ({ deps }) => {
    switch (deps.view) {
      case "body":
        return { view: "body" as const, data: await loadBodyProfile(deps) }
      case "progress":
        return { view: "progress" as const, data: await loadProgress(deps) }
      case "log":
      case "today":
      case undefined:
        return { view: "log" as const, data: await loadWorkouts(deps) }
    }
  },
  staleTime: 0,
  headers: () => ({ "Cache-Control": "no-store" }),
  component: WorkoutPage,
})

const views = [
  { view: undefined, label: "Log", Icon: BarbellIcon },
  { view: "progress", label: "Progress", Icon: ChartLineUpIcon },
  { view: "body", label: "Body", Icon: ScalesIcon },
] as const

function WorkoutPage(): React.JSX.Element {
  const data = Route.useLoaderData()
  const search = Route.useSearch()
  const navigate = Route.useNavigate()
  const router = useRouter()
  const mutateBody = useServerFn(mutateBodyProfile)

  const switcher = (
    <nav className="view-tabs" aria-label="Workout views">
      {views.map((view) => (
        <Link
          key={view.label}
          to="/workout"
          search={(previous) => ({ ...previous, view: view.view })}
          activeOptions={{
            exact: true,
            includeSearch: true,
            explicitUndefined: true,
          }}
          className="view-tab"
          aria-current={data.view === (view.view ?? "log") ? "page" : undefined}
        >
          <view.Icon size={20} weight="bold" aria-hidden="true" />
          {view.label}
        </Link>
      ))}
    </nav>
  )

  return (
    <section>
      <PageHead title="Workout" />
      {data.view === "progress" ? (
        <ExerciseProgress
          lead={switcher}
          data={data.data}
          choosePeriod={(period) =>
            navigate({
              search: (previous) => ({ ...previous, period }),
              resetScroll: false,
            })
          }
          compare={(ids) =>
            navigate({
              search: (previous) => ({
                ...previous,
                compare: ids.length === 0 ? undefined : [...ids],
              }),
              resetScroll: false,
            })
          }
        />
      ) : null}
      {data.view === "body" ? (
        <BodyProfile
          lead={switcher}
          data={data.data}
          range={{ from: search.from, to: search.to }}
          change={(command) => mutateBody({ data: command })}
          reload={() => router.invalidate({ sync: true })}
          filter={async (range) => {
            if (range.from === search.from && range.to === search.to)
              await router.invalidate({ sync: true })
            else
              await navigate({
                search: (previous) => ({ ...previous, ...range }),
                resetScroll: false,
              })
          }}
        />
      ) : null}
      {data.view === "log" ? (
        <Workouts
          lead={switcher}
          data={data.data}
          showOlder={() =>
            navigate({
              search: (previous) => ({
                ...previous,
                shown: Math.min(
                  maxHistory,
                  (previous.shown ?? historyStep) + historyStep
                ),
              }),
              resetScroll: false,
            })
          }
        />
      ) : null}
    </section>
  )
}
