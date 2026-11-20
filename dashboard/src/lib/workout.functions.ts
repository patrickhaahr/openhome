import { createServerFn } from "@tanstack/react-start"
import { setResponseHeader } from "@tanstack/react-start/server"
import { workoutSearch, type WorkoutSearch, type WorkoutState } from "./workout"

export const getWorkouts = createServerFn({ method: "GET" })
  .validator(workoutSearch)
  .handler(async ({ data }) => {
    setResponseHeader("Cache-Control", "no-store")
    const { readWorkouts } = await import("./workout.server")

    return readWorkouts(data)
  })

export async function loadWorkouts(
  search: WorkoutSearch
): Promise<WorkoutState> {
  try {
    return await getWorkouts({ data: search })
  } catch {
    return { state: "unavailable", reason: "unreachable" }
  }
}
