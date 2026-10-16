import { createFileRoute, Link, useRouter } from "@tanstack/react-router"
import { useServerFn } from "@tanstack/react-start"
import { BarbellIcon } from "@phosphor-icons/react/dist/csr/Barbell"
import { ChartLineUpIcon } from "@phosphor-icons/react/dist/csr/ChartLineUp"
import { ScalesIcon } from "@phosphor-icons/react/dist/csr/Scales"
import { TargetIcon } from "@phosphor-icons/react/dist/csr/Target"
import { z } from "zod"
import { BodyProfile } from "@/components/body-profile"
import { PageHead } from "@/components/page-head"
import { ExerciseProgress } from "@/components/exercise-progress"
import { Workouts } from "@/components/workouts"
import { TrainingSummaries } from "@/components/training-summaries"
import { trainingSearch } from "@/lib/training"
import { loadTraining } from "@/lib/training.functions"
import { bodyRange } from "@/lib/body-profile"
import {
  loadBodyProfile,
  mutateBodyProfile,
} from "@/lib/body-profile.functions"
import { progressSearch } from "@/lib/exercise-progress"
import { loadProgress } from "@/lib/exercise-progress.functions"
import { historyStep, maxHistory, workoutSearch } from "@/lib/workout"
import { loadWorkouts, mutateWorkout } from "@/lib/workout.functions"

/** Workout views: today's recovery first, then the log, exercise progress and body weight. */
export const Route = createFileRoute("/workout")({
  head: () => ({ meta: [{ title: "Workout · OpenHome" }] }),
  validateSearch: workoutSearch.extend({
    view: z.enum(["log", "progress", "body"]).optional().catch(undefined),
    ...bodyRange.shape,
    ...progressSearch.shape,
    ...trainingSearch.shape,
  }),
  loaderDeps: ({ search }) => ({
    view: search.view,
    from: search.from,
    to: search.to,
    workout: search.workout,
    shown: search.shown,
    period: search.period,
    compare: search.compare,
    day: search.day,
    noop: search.noop,
  }),
  loader: async ({ deps }) => {
    switch (deps.view) {
      case "body":
        return { view: "body" as const, data: await loadBodyProfile(deps) }
      case "progress":
        return { view: "progress" as const, data: await loadProgress(deps) }
      case "log":
        return { view: "log" as const, data: await loadWorkouts(deps) }
      case undefined:
        return { view: "today" as const, data: await loadTraining(deps) }
    }
  },
  staleTime: 0,
  headers: () => ({ "Cache-Control": "no-store" }),
  component: WorkoutPage,
})

const views = [
  { view: undefined, label: "Today", Icon: TargetIcon },
  { view: "log", label: "Log", Icon: BarbellIcon },
  { view: "progress", label: "Progress", Icon: ChartLineUpIcon },
  { view: "body", label: "Body", Icon: ScalesIcon },
] as const

function WorkoutPage(): React.JSX.Element {
  const data = Route.useLoaderData()
  const search = Route.useSearch()
  const navigate = Route.useNavigate()
  const router = useRouter()
  const mutate = useServerFn(mutateWorkout)
  const mutateBody = useServerFn(mutateBodyProfile)

  return (
    <section>
      <PageHead title="Workout" />
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
            aria-current={
              data.view === (view.view ?? "today") ? "page" : undefined
            }
          >
            <view.Icon size={20} weight="bold" aria-hidden="true" />
            {view.label}
          </Link>
        ))}
      </nav>
      {data.view === "progress" ? (
        <ExerciseProgress
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
          refresh={() => router.invalidate({ sync: true })}
        />
      ) : null}
      {data.view === "body" ? (
        <BodyProfile
          data={data.data}
          range={{ from: search.from, to: search.to }}
          change={(command) => mutateBody({ data: command })}
          refresh={() => router.invalidate({ sync: true })}
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
      {data.view === "today" ? (
        <TrainingSummaries
          key={data.data.day}
          data={data.data}
          refresh={() => router.invalidate({ sync: true })}
          selectDay={async (day) => {
            if (day === data.data.day && search.noop === undefined)
              await router.invalidate({ sync: true })
            else
              await navigate({
                search: (previous) => ({ ...previous, day, noop: undefined }),
                resetScroll: false,
              })
          }}
          openLogged={(workout) =>
            navigate({ search: { view: "log", workout } })
          }
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
      ) : null}
      {data.view === "log" ? (
        <Workouts
          data={data.data}
          change={(command) => mutate({ data: command })}
          open={async (workout) => {
            // A changed selection reloads through loaderDeps; the same one needs a fresh read.
            if (workout === search.workout)
              await router.invalidate({ sync: true })
            else
              await navigate({
                search: (previous) => ({ ...previous, workout }),
                resetScroll: false,
              })
          }}
          refresh={() => router.invalidate({ sync: true })}
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
