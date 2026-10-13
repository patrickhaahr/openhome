import "@tanstack/react-start/server-only"
import { z } from "zod"
import { requestOpenHome } from "./openhome.server"
import { trainingDayOn } from "./workout"
import {
  noopWorkoutDetail,
  recoveryDay,
  trainingContext,
  type TrainingRead,
  type TrainingSearch,
  type TrainingState,
} from "./training"

async function read<T>(
  path: `/api/${string}`,
  schema: z.ZodType<T>
): Promise<TrainingRead<T>> {
  const result = await requestOpenHome({ path, method: "GET", timeoutMs: 5000 })

  if (result.state !== "received")
    return { state: "unavailable", reason: result.state }
  const response = result.response

  if (!response.ok) {
    const reason =
      response.status === 422
        ? "too-large"
        : response.status === 404
          ? "not-found"
          : response.status === 400 || response.status === 409
            ? "invalid"
            : "unavailable"

    return { state: "unavailable", reason }
  }

  try {
    const parsed = schema.safeParse(await response.json())

    return parsed.success
      ? { state: "available", data: parsed.data }
      : { state: "unavailable", reason: "unavailable" }
  } catch {
    return { state: "unavailable", reason: "unavailable" }
  }
}

/** Read existing Axum training models in parallel; selection and wearable interpretation remain in Axum. */
export async function readTraining(
  search: TrainingSearch
): Promise<TrainingState> {
  const day = search.day ?? trainingDayOn(new Date())
  // Calendar arithmetic for a seven-day UI window, not elapsed local hours across DST.
  const from = new Date(`${day}T00:00:00Z`)
  from.setUTCDate(from.getUTCDate() - 6)
  const selected = search.noop

  // Literal dot segments are normalized even when URL-encoded; never let them change the API path.
  const safeSegment = (value: string) =>
    // oxlint-disable-next-line no-control-regex -- Axum rejects control characters in path identities.
    value !== "." && value !== ".." && !/[\u0000-\u001f\u007f]/.test(value)

  const detail =
    selected === undefined
      ? Promise.resolve({ state: "none" } as const)
      : !safeSegment(selected.source) || !safeSegment(selected.sport)
        ? Promise.resolve({ state: "unavailable", reason: "invalid" } as const)
        : read(
            `/api/training/noop-workouts/${encodeURIComponent(selected.source)}/${encodeURIComponent(selected.start)}/${encodeURIComponent(selected.sport)}`,
            noopWorkoutDetail
          )

  const [recovery, context, selectedDetail] = await Promise.all([
    read(`/api/training/days/${day}/recovery`, recoveryDay),
    read(
      `/api/training/context/${from.toISOString().slice(0, 10)}/${day}`,
      trainingContext
    ),
    detail,
  ])

  return { day, recovery, context, detail: selectedDetail }
}
