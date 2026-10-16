import { createServerFn } from "@tanstack/react-start"
import { setResponseHeader } from "@tanstack/react-start/server"
import { z } from "zod"
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

/** Read a run of Recovery Days for the day strip; at most 90 days, oldest first. */
export const getRecoveryDays = createServerFn({ method: "GET" })
  .validator(z.object({ from: z.iso.date(), to: z.iso.date() }))
  .handler(async ({ data }) => {
    setResponseHeader("Cache-Control", "no-store")
    const { readRecoveryDays } = await import("./training.server")

    return readRecoveryDays(data.from, data.to)
  })

/** Read today's Recovery Day using the server-held Axum credential. */
export const getRecoveryToday = createServerFn({ method: "GET" }).handler(
  async () => {
    setResponseHeader("Cache-Control", "no-store")
    const { readRecoveryToday } = await import("./training.server")

    return readRecoveryToday()
  }
)

/** A dropped RPC becomes a recoverable view with no fictional empty wearable data. */
export async function loadTraining(
  search: TrainingSearch
): Promise<TrainingState> {
  try {
    return await getTraining({ data: search })
  } catch {
    const failure = { state: "unavailable", reason: "unreachable" } as const

    const today = trainingDayOn(new Date())

    return {
      day: search.day ?? today,
      today,
      recovery: failure,
      week: [],
      trends: {
        hrv: failure,
        resting_heart_rate: failure,
        sleep_duration: failure,
      },
      context: failure,
      detail: search.noop === undefined ? { state: "none" } : failure,
    }
  }
}
