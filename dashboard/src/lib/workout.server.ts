import "@tanstack/react-start/server-only"
import { z } from "zod"
import { requestOpenHome } from "./openhome.server"
import {
  exercise,
  historyStep,
  workoutDetail,
  workoutSummary,
  type SelectedWorkout,
  type WorkoutCommand,
  type WorkoutFailure,
  type WorkoutMutation,
  type WorkoutSearch,
  type WorkoutState,
} from "./workout"

function reason(status: number): WorkoutFailure {
  if (status === 400) return "invalid"

  // Axum answers 422 only when an entry names an Exercise that is not stored.
  if (status === 422) return "missing-exercise"

  if (status === 404) return "not-found"

  return "unavailable"
}

type Read<T> =
  | { readonly state: "read"; readonly data: T }
  | { readonly state: "failed"; readonly reason: WorkoutFailure }

async function read<T>(
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

/**
 * Read the exercise library, recent logged Workouts and one selected Workout from authenticated Axum.
 *
 * Without a selection the newest Workout is shown. Incomplete reads never become an empty history.
 *
 * @param search - The selected Workout and how much history to list.
 * @returns The training view, or a sanitized unavailable outcome.
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

  const selectedId = search.workout ?? history.data[0]?.id
  let selected: SelectedWorkout = { state: "none" }

  if (selectedId !== undefined) {
    const detail = await read(`/api/workouts/${selectedId}`, workoutDetail)

    if (detail.state === "read")
      selected = { state: "found", workout: detail.data }
    else if (detail.reason === "not-found") selected = { state: "missing" }
    else return { state: "unavailable", reason: detail.reason }
  }

  return {
    state: "available",
    exercises: exercises.data,
    history: history.data.slice(0, shown),
    hasOlder: history.data.length > shown,
    selected,
  }
}

/**
 * Forward one Workout write; only a parsed Axum receipt confirms persistence.
 *
 * Axum validates and replaces entries in one transaction, so a rejected edit leaves the stored Workout unchanged.
 *
 * @param command - The create, full replacement or removal.
 * @returns The stored Workout id, a removal receipt, or a sanitized failure.
 */
export async function writeWorkout(
  command: WorkoutCommand
): Promise<WorkoutMutation> {
  if (command.kind === "remove") {
    const result = await requestOpenHome({
      path: `/api/workouts/${command.workout_id}`,
      method: "DELETE",
      timeoutMs: 5000,
    })

    if (result.state !== "received")
      return { state: "failed", reason: result.state }

    if (result.response.status === 204) return { state: "removed" }

    return {
      state: "failed",
      reason: result.response.ok
        ? "unavailable"
        : reason(result.response.status),
    }
  }

  const { date, name, notes, exercises } = command

  const result = await requestOpenHome({
    ...(command.kind === "create"
      ? { path: "/api/workouts", method: "POST" }
      : { path: `/api/workouts/${command.workout_id}`, method: "PATCH" }),
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
