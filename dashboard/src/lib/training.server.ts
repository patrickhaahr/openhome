import "@tanstack/react-start/server-only"
import { z } from "zod"
import { requestOpenHome } from "./openhome.server"
import { trainingDayOn } from "./workout"
import {
  metricTrend,
  noopWorkoutDetail,
  recoveryDay,
  shiftDay,
  trainingContext,
  stripDays,
  trendDays,
  type RecoveryDay,
  type TrendMetric,
  type TrainingRead,
  type TrainingSearch,
  type TrainingState,
} from "./training"

async function read<T>(
  path: `/api/${string}`,
  schema: z.ZodType<T>,
  query?: Record<string, string>
): Promise<TrainingRead<T>> {
  const result = await requestOpenHome({
    path,
    method: "GET",
    timeoutMs: 5000,
    query,
  })

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

/** The earlier of two Training Days. */
function minDay(a: string, b: string): string {
  return a < b ? a : b
}

/** Recovery Days from `from` through `to`, oldest first, leaving out `skip` (read by the caller). */
export async function readRecoveryDays(
  from: string,
  to: string,
  skip?: string
): Promise<TrainingState["week"]> {
  const days: string[] = []

  for (
    let next = from;
    next <= to && days.length < 90;
    next = shiftDay(next, 1)
  )
    if (next !== skip) days.push(next)

  return Promise.all(
    days.map(async (day) => ({
      day,
      recovery: await read(`/api/training/days/${day}/recovery`, recoveryDay),
    }))
  )
}

/** Read existing Axum training models in parallel; selection and wearable interpretation remain in Axum. */
export async function readTraining(
  search: TrainingSearch
): Promise<TrainingState> {
  const today = trainingDayOn(new Date())
  const day = search.day ?? today
  const from = shiftDay(day, -6)
  // The day strip opens on a month around the selected day and fills toward today and the past.
  const weekEnd = day >= today ? day : minDay(today, shiftDay(day, 13))
  const weekStart = shiftDay(weekEnd, 1 - stripDays)
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
            noopWorkoutDetail,
            { route: "true" }
          )

  const trendFrom = shiftDay(day, 1 - trendDays)

  const recoveryOn = (weekDay: string) =>
    read(`/api/training/days/${weekDay}/recovery`, recoveryDay)

  const trend = (metric: TrendMetric) =>
    read(`/api/training/trends/${metric}/${trendFrom}/${day}`, metricTrend)

  // Axum scores one Recovery Day per read; the strip reads the days around the selected one.
  const [others, recovery, hrv, restingHr, sleep, context, selectedDetail] =
    await Promise.all([
      readRecoveryDays(weekStart, weekEnd, day),
      recoveryOn(day),
      trend("hrv"),
      trend("resting_heart_rate"),
      trend("sleep_duration"),
      read(`/api/training/context/${from}/${day}`, trainingContext),
      detail,
    ])

  return {
    day,
    today,
    recovery,
    week: [...others, { day, recovery }].sort((a, b) =>
      a.day.localeCompare(b.day)
    ),
    trends: { hrv, resting_heart_rate: restingHr, sleep_duration: sleep },
    context,
    detail: selectedDetail,
  }
}

/** Today's Recovery Day alone, for Home's glance. */
export async function readRecoveryToday(): Promise<TrainingRead<RecoveryDay>> {
  return read(
    `/api/training/days/${trainingDayOn(new Date())}/recovery`,
    recoveryDay
  )
}
