import "@tanstack/react-start/server-only"
import {
  combinedProgress,
  periodBounds,
  type ProgressQuery,
  type ProgressSearch,
  type ProgressState,
} from "./exercise-progress"
import { exercise, trainingDayOn } from "./workout"
import { read } from "./workout.server"

export async function readProgress(
  search: ProgressSearch
): Promise<ProgressState> {
  const period = search.period ?? "4w"
  const requested = search.compare ?? []

  const library =
    requested.length === 0
      ? undefined
      : await read("/api/exercises", exercise.array())

  if (library !== undefined && library.state !== "read")
    return { state: "unavailable", reason: library.reason }

  const known = new Set(library?.data.map((entry) => entry.id))

  const compared = requested.filter((id) => known.has(id))

  const bounds = periodBounds({ period, today: trainingDayOn(new Date()) })

  const query: ProgressQuery =
    compared.length === 0
      ? bounds
      : { ...bounds, exercise_ids: compared.join(",") }

  const progress = await read(
    "/api/exercises/progress",
    combinedProgress,
    query
  )

  if (progress.state !== "read")
    return { state: "unavailable", reason: progress.reason }

  return {
    state: "available",
    period,
    progress: progress.data,
    compared,
  }
}
