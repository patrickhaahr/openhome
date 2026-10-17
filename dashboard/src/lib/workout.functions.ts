import { createServerFn } from "@tanstack/react-start"
import { setResponseHeader } from "@tanstack/react-start/server"
import { workoutSearch, type WorkoutSearch, type WorkoutState } from "./workout"

/** Read the shared exercise library and logged Workouts through the server-held Axum credential. */
export const getWorkouts = createServerFn({ method: "GET" })
  .validator(workoutSearch)
  .handler(async ({ data }) => {
    setResponseHeader("Cache-Control", "no-store")
    const { readWorkouts } = await import("./workout.server")

    return readWorkouts(data)
  })

/**
 * Read the training view; a dropped read RPC renders recovery controls instead of a route error.
 *
 * @param search - The selected Workout and how much history to list.
 * @returns The training view, or an unreachable outcome.
 */
export async function loadWorkouts(
  search: WorkoutSearch
): Promise<WorkoutState> {
  try {
    return await getWorkouts({ data: search })
  } catch {
    return { state: "unavailable", reason: "unreachable" }
  }
}
