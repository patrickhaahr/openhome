import { createServerFn } from "@tanstack/react-start"
import { setResponseHeader } from "@tanstack/react-start/server"
import {
  trainingSearch,
  type TrainingSearch,
  type TrainingState,
} from "./training"
import { trainingDayOn } from "./workout"

/** Read private training data using the server-held Axum credential. */
export const getTraining = createServerFn({ method: "GET" })
  .validator(trainingSearch)
  .handler(async ({ data }) => {
    setResponseHeader("Cache-Control", "no-store")
    const { readTraining } = await import("./training.server")

    return readTraining(data)
  })

/** A dropped RPC becomes a recoverable view with no fictional empty wearable data. */
export async function loadTraining(
  search: TrainingSearch
): Promise<TrainingState> {
  try {
    return await getTraining({ data: search })
  } catch {
    const failure = { state: "unavailable", reason: "unreachable" } as const

    return {
      day: search.day ?? trainingDayOn(new Date()),
      recovery: failure,
      context: failure,
      detail: search.noop === undefined ? { state: "none" } : failure,
    }
  }
}
