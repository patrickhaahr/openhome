import { createServerFn } from "@tanstack/react-start"
import { setResponseHeader } from "@tanstack/react-start/server"
import {
  progressSearch,
  type ProgressSearch,
  type ProgressState,
} from "./exercise-progress"

/** Read combined exercise progress through the server-held Axum credential. */
export const getProgress = createServerFn({ method: "GET" })
  .validator(progressSearch)
  .handler(async ({ data }) => {
    setResponseHeader("Cache-Control", "no-store")
    const { readProgress } = await import("./exercise-progress.server")

    return readProgress(data)
  })

/**
 * Read the progress view; a dropped read RPC renders recovery controls instead of a route error.
 *
 * @param search - The period and compared Exercises.
 * @returns The chart data, or an unreachable outcome.
 */
export async function loadProgress(
  search: ProgressSearch
): Promise<ProgressState> {
  try {
    return await getProgress({ data: search })
  } catch {
    return { state: "unavailable", reason: "unreachable" }
  }
}
