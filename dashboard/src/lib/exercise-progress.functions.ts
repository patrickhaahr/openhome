import { createServerFn } from "@tanstack/react-start"
import { setResponseHeader } from "@tanstack/react-start/server"
import {
  progressSearch,
  type ProgressSearch,
  type ProgressState,
} from "./exercise-progress"

export const getProgress = createServerFn({ method: "GET" })
  .validator(progressSearch)
  .handler(async ({ data }) => {
    setResponseHeader("Cache-Control", "no-store")
    const { readProgress } = await import("./exercise-progress.server")

    return readProgress(data)
  })

export async function loadProgress(
  search: ProgressSearch
): Promise<ProgressState> {
  try {
    return await getProgress({ data: search })
  } catch {
    return { state: "unavailable", reason: "unreachable" }
  }
}
