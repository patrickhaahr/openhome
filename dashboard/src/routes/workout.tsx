import { createFileRoute, useRouter } from "@tanstack/react-router"
import { useServerFn } from "@tanstack/react-start"
import { Workouts } from "@/components/workouts"
import { historyStep, maxHistory, workoutSearch } from "@/lib/workout"
import { loadWorkouts, mutateWorkout } from "@/lib/workout.functions"

/** Logged Workouts and the exercise library, read and written through Axum. */
export const Route = createFileRoute("/workout")({
  head: () => ({ meta: [{ title: "Workout · OpenHome" }] }),
  validateSearch: workoutSearch,
  loaderDeps: ({ search }) => ({
    workout: search.workout,
    shown: search.shown,
  }),
  loader: ({ deps }) => loadWorkouts(deps),
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

  return (
    <section className="focused-page workout-page">
      <header>
        <p className="page-eyebrow">OpenHome / Workout</p>
        <h1 id="page-heading" tabIndex={-1}>
          Workout
        </h1>
        <p className="feature-intro">
          Log the sets you train and look back on earlier sessions.
        </p>
      </header>
      <Workouts
        data={data}
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
    </section>
  )
}
