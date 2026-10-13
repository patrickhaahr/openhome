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

/**
 * Read the combined Progress Lines for the chosen period from authenticated Axum.
 *
 * Compared Exercises removed from the library are dropped before the batch read, so one stale
 * id cannot hide every other line; with none left, Axum picks the most recently logged ones.
 *
 * @param search - The period and compared Exercises.
 * @returns The chart data, or a sanitized unavailable outcome.
 */
export async function readProgress(
  search: ProgressSearch
): Promise<ProgressState> {
  const period = search.period ?? "3m"
  const library = await read("/api/exercises", exercise.array())

  if (library.state !== "read")
    return { state: "unavailable", reason: library.reason }

  const known = new Set(library.data.map((entry) => entry.id))

  const compared = (search.compare ?? []).filter((id) => known.has(id))

  const bounds = periodBounds(period, trainingDayOn(new Date()))

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
    library: library.data,
    compared,
  }
}
