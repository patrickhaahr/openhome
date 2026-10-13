import { createFileRoute, Link, useRouter } from "@tanstack/react-router"
import { useServerFn } from "@tanstack/react-start"
import { z } from "zod"
import { BodyProfile } from "@/components/body-profile"
import { buttonVariants } from "@/components/ui/button"
import { Workouts } from "@/components/workouts"
import { TrainingSummaries } from "@/components/training-summaries"
import { trainingSearch } from "@/lib/training"
import { loadTraining } from "@/lib/training.functions"
import { bodyRange } from "@/lib/body-profile"
import {
  loadBodyProfile,
  mutateBodyProfile,
} from "@/lib/body-profile.functions"
import { historyStep, maxHistory, workoutSearch } from "@/lib/workout"
import { loadWorkouts, mutateWorkout } from "@/lib/workout.functions"

/** Logged Workouts and the exercise library, read and written through Axum. */
export const Route = createFileRoute("/workout")({
  head: () => ({ meta: [{ title: "Workout · OpenHome" }] }),
  validateSearch: workoutSearch.extend({
    view: z.enum(["body", "recovery"]).optional().catch(undefined),
    ...bodyRange.shape,
    ...trainingSearch.shape,
  }),
  loaderDeps: ({ search }) => ({
    view: search.view,
    from: search.from,
    to: search.to,
    workout: search.workout,
    shown: search.shown,
    day: search.day,
    noop: search.noop,
  }),
  loader: async ({ deps }) =>
    deps.view === "body"
      ? { view: "body" as const, data: await loadBodyProfile(deps) }
      : deps.view === "recovery"
        ? { view: "recovery" as const, data: await loadTraining(deps) }
        : { view: "training" as const, data: await loadWorkouts(deps) },
  staleTime: 0,
  headers: () => ({ "Cache-Control": "no-store" }),
  component: WorkoutPage,
})

function WorkoutPage(): React.JSX.Element {
  const data = Route.useLoaderData()
  const search = Route.useSearch()
  const navigate = Route.useNavigate()
  const router = useRouter()
  const mutate = useServerFn(mutateWorkout)
  const mutateBody = useServerFn(mutateBodyProfile)

  return (
    <section className="focused-page workout-page">
      <header>
        <p className="page-eyebrow">OpenHome / Workout</p>
        <h1 id="page-heading" tabIndex={-1}>
          Workout
        </h1>
        <p className="feature-intro">
          Log your training, review earlier sessions and keep your profile up to
          date. Explore NOOP recovery and recorded sessions alongside your
          training.
        </p>
      </header>
      <nav className="workout-view-nav" aria-label="Workout views">
        <Link
          to="/workout"
          search={(previous) => ({ ...previous, view: undefined })}
          activeOptions={{
            exact: true,
            includeSearch: true,
            explicitUndefined: true,
          }}
          className={buttonVariants({
            variant: data.view === "training" ? "secondary" : "ghost",
            size: "lg",
          })}
        >
          Workouts
        </Link>
        <Link
          to="/workout"
          search={(previous) => ({ ...previous, view: "recovery" })}
          activeOptions={{ exact: true, includeSearch: true }}
          className={buttonVariants({
            variant: data.view === "recovery" ? "secondary" : "ghost",
            size: "lg",
          })}
        >
          Recovery & NOOP
        </Link>
        <Link
          to="/workout"
          search={(previous) => ({ ...previous, view: "body" })}
          activeOptions={{ exact: true, includeSearch: true }}
          className={buttonVariants({
            variant: data.view === "body" ? "secondary" : "ghost",
            size: "lg",
          })}
        >
          Profile & weight
        </Link>
      </nav>
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
      ) : data.view === "recovery" ? (
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
      ) : (
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
      )}
    </section>
  )
}
