import "@tanstack/react-start/server-only"
import { z } from "zod"
import { requestOpenHome } from "./openhome.server"
import { pairSessions } from "./session-log"
import {
  noopWorkoutDetail,
  shiftDay,
  trainingContext,
  type NoopSummary,
} from "./training"
import { noopDetailPath } from "./training.server"
import {
  exercise,
  historyStep,
  maxHistory,
  workoutDetail,
  workoutSummary,
  trainingDayOn,
  type LogSession,
  type WorkoutDetail,
  type WorkoutCommand,
  type WorkoutFailure,
  type WorkoutMutation,
  type WorkoutSearch,
  type WorkoutState,
} from "./workout"

function reason(status: number): WorkoutFailure {
  if (status === 400) return "invalid"

  // Axum answers 422 when an entry names an Exercise that is not stored; typed writes rule out body-shape 422s.
  if (status === 422) return "missing-exercise"

  if (status === 404) return "not-found"

  return "unavailable"
}

/** One authenticated Axum read, or a sanitized failure; never a partial value. */
export type Read<T> =
  | { readonly state: "read"; readonly data: T }
  | { readonly state: "failed"; readonly reason: WorkoutFailure }

/**
 * Read and parse one authenticated Axum fitness resource.
 *
 * @param path - The Axum path.
 * @param schema - The response contract.
 * @param query - Query parameters.
 * @returns The parsed body, or a sanitized failure class.
 */
export async function read<T>(
  path: `/api/${string}`,
  schema: z.ZodType<T>,
  query: Record<string, string> = {}
): Promise<Read<T>> {
  const result = await requestOpenHome({
    path,
    method: "GET",
    timeoutMs: 5000,
    query,
  })

  if (result.state !== "received")
    return { state: "failed", reason: result.state }

  if (!result.response.ok)
    return { state: "failed", reason: reason(result.response.status) }

  try {
    const parsed = schema.safeParse(await result.response.json())

    return parsed.success
      ? { state: "read", data: parsed.data }
      : { state: "failed", reason: "unavailable" }
  } catch {
    return { state: "failed", reason: "unavailable" }
  }
}

function minDay(a: string, b: string): string {
  return a < b ? a : b
}

/** Days one Training Context read covers at most. */
const contextDays = 90

/** A quiet month of strap sessions is shown even when no Workout was logged in it. */
const minimumDays = 28

/** Strap detail reads in flight at once; each is a small read of the NOOP mirror. */
const detailBatch = 8

async function inBatches<T, R>(
  items: ReadonlyArray<T>,
  work: (item: T) => Promise<R>
): Promise<R[]> {
  const results: R[] = []

  for (let at = 0; at < items.length; at += detailBatch)
    results.push(
      ...(await Promise.all(items.slice(at, at + detailBatch).map(work)))
    )

  return results
}

/** The strap's sessions from `from` through `to`, or undefined when any part could not be read. */
async function readStrapSessions(
  from: string,
  to: string
): Promise<NoopSummary[] | undefined> {
  const ranges: Array<readonly [string, string]> = []

  for (let end = to; end >= from; end = shiftDay(end, -contextDays)) {
    const start = shiftDay(end, 1 - contextDays)

    ranges.push([start < from ? from : start, end])
  }

  const reads = await Promise.all(
    ranges.map(([start, end]) =>
      read(`/api/training/context/${start}/${end}`, trainingContext)
    )
  )

  const sessions: NoopSummary[] = []

  for (const context of reads) {
    if (context.state !== "read") return undefined

    for (const day of context.data.days) sessions.push(...day.noop_workouts)
  }

  return sessions
}

/**
 * Read the exercise library and the Log: recent logged Workouts with their sets, the strap's
 * sessions over the same days, each strap session's detail, and the entries they pair into.
 *
 * Incomplete reads of OpenHome never become an empty history; a strap that cannot be read only
 * leaves the logged Workouts standing alone.
 *
 * @param search - How much history to list.
 * @returns The Log, or a sanitized unavailable outcome.
 */
export async function readWorkouts(
  search: WorkoutSearch
): Promise<WorkoutState> {
  const shown = search.shown ?? historyStep

  // One extra row reveals whether older Workouts exist without a count endpoint.
  const [exercises, history] = await Promise.all([
    read("/api/exercises", exercise.array()),
    read("/api/workouts", workoutSummary.array(), {
      limit: String(shown + 1),
    }),
  ])

  if (exercises.state !== "read")
    return { state: "unavailable", reason: exercises.reason }

  if (history.state !== "read")
    return { state: "unavailable", reason: history.reason }

  const hasOlder = history.data.length > shown && shown < maxHistory

  const details = await Promise.all(
    history.data
      .slice(0, shown)
      .map((entry) => read(`/api/workouts/${entry.id}`, workoutDetail))
  )

  const workouts: WorkoutDetail[] = []

  for (const detail of details) {
    if (detail.state === "read") workouts.push(detail.data)
    // Removed since the list was read: leave it out rather than fail the Log.
    else if (detail.reason !== "not-found")
      return { state: "unavailable", reason: detail.reason }
  }

  const today = trainingDayOn(new Date())
  const oldest = workouts.reduce((day, entry) => minDay(day, entry.date), today)

  const from = hasOlder
    ? oldest
    : minDay(oldest, shiftDay(today, 1 - minimumDays))

  const summaries = await readStrapSessions(from, today)

  const paired = pairSessions(workouts, summaries ?? [])

  // Routes are only for sessions that stand alone; a paired session shows its exercises instead.
  const reads = await inBatches(paired, async (session) => {
    const path =
      session.strap === undefined ? undefined : noopDetailPath(session.strap)

    return path === undefined
      ? undefined
      : read(
          path,
          noopWorkoutDetail,
          session.workout === undefined ? { route: "true" } : {}
        )
  })

  const sessions = paired.map((session, position): LogSession => {
    const result = reads[position]

    const detail = result?.state === "read" ? result.data : undefined

    return {
      ...session,
      key:
        session.strap === undefined
          ? `workout-${session.workout?.id ?? position}`
          : `strap-${session.strap.source}-${session.strap.start}-${session.strap.sport}`,
      strap:
        session.strap === undefined
          ? undefined
          : { summary: session.strap, detail },
    }
  })

  return {
    state: "available",
    exercises: exercises.data,
    sessions,
    hasOlder,
    strap: summaries === undefined ? "unavailable" : "read",
  }
}

/**
 * Forward one Workout edit; only a parsed Axum receipt confirms persistence.
 *
 * Axum validates and replaces entries in one transaction, so a rejected edit leaves the stored Workout unchanged.
 *
 * @param command - The full replacement.
 * @returns The stored Workout id or a sanitized failure.
 */
export async function writeWorkout(
  command: WorkoutCommand
): Promise<WorkoutMutation> {
  const { date, name, notes, exercises } = command

  const result = await requestOpenHome({
    path: `/api/workouts/${command.workout_id}`,
    method: "PATCH",
    json: { date, name, notes, exercises },
    timeoutMs: 5000,
  })

  if (result.state !== "received")
    return { state: "failed", reason: result.state }

  if (!result.response.ok)
    return { state: "failed", reason: reason(result.response.status) }

  try {
    const parsed = workoutDetail.safeParse(await result.response.json())

    if (parsed.success) return { state: "saved", workoutId: parsed.data.id }
  } catch {
    /* Never expose upstream error bodies. */
  }

  return { state: "failed", reason: "unavailable" }
}
