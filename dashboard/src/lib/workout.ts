import { z } from "zod"
import type { NoopSummary, NoopWorkoutDetail } from "./training"

const id = z.number().int()

const exerciseCategory = z.enum(["calisthenics", "gym"])

/** One movement in the shared exercise library. */
export const exercise = z.object({
  id,
  name: z.string(),
  category: exerciseCategory,
  muscle_group: z.string().nullable(),
  equipment: z.string().nullable(),
})

/** A library Exercise as Axum serves it. */
export type Exercise = z.infer<typeof exercise>

/** A logged Workout in history order, without its entries. */
export const workoutSummary = z.object({
  id,
  date: z.string(),
  name: z.string().nullable(),
})

/** One row of logged-Workout history. */
export type WorkoutSummary = z.infer<typeof workoutSummary>

const loggedSet = z.object({
  id,
  set_number: z.number().int(),
  reps: z.number().int().nullable(),
  weight_kg: z.number().nullable(),
  duration_seconds: z.number().int().nullable(),
  rpe: z.number().int().nullable(),
  notes: z.string().nullable(),
})

/** One stored Set: reps or a Timed Hold, with Added Weight null for bodyweight. */
export type LoggedSet = z.infer<typeof loggedSet>

const workoutEntry = z.object({
  id,
  order_index: z.number().int(),
  notes: z.string().nullable(),
  exercise: z.object({ id, name: z.string(), category: exerciseCategory }),
  sets: loggedSet.array(),
})

/** One ordered Exercise entry of a stored Workout. */
export type WorkoutEntry = z.infer<typeof workoutEntry>

/** A stored Workout with ordered entries and Sets. */
export const workoutDetail = z.object({
  id,
  date: z.string(),
  name: z.string().nullable(),
  notes: z.string().nullable(),
  exercises: workoutEntry.array(),
})

/** A logged Workout as Axum returns it after reads and writes. */
export type WorkoutDetail = z.infer<typeof workoutDetail>

/** History pages grow in fixed steps, bounded below Axum's 500-row list limit. */
export const historyStep = 20

/** The longest history view; the last page offers no older rows. */
export const maxHistory = 480

/** How many logged Workouts the Log lists; invalid values fall back to the first page. */
export const workoutSearch = z.object({
  shown: z
    .number()
    .int()
    .min(historyStep)
    .max(maxHistory)
    .optional()
    .catch(undefined),
})

/** Parsed `/workout` search parameters. */
export type WorkoutSearch = z.infer<typeof workoutSearch>

/** Sanitized failure classes; upstream diagnostic text never reaches the browser. */
export type WorkoutFailure =
  | "missing-configuration"
  | "invalid-configuration"
  | "rejected-credential"
  | "unreachable"
  | "unavailable"
  | "invalid"
  | "missing-exercise"
  | "not-found"

/** A NOOP Workout in the Log: the summary Axum lists, and its detail when that read succeeded. */
export type Strap = {
  readonly summary: NoopSummary
  readonly detail: NoopWorkoutDetail | undefined
}

/** One Log entry: a logged Workout, a strap session, or the two as one session. */
export type LogSession = {
  /** Stable across reads: the Workout id, else the strap identity. */
  readonly key: string
  /** The Copenhagen Training Day the entry sits on. */
  readonly day: string
  readonly workout: WorkoutDetail | undefined
  readonly strap: Strap | undefined
}

/** Complete training read, or an outcome that must never look like an empty history. */
export type WorkoutState =
  | {
      readonly state: "available"
      readonly exercises: Exercise[]
      readonly sessions: LogSession[]
      readonly hasOlder: boolean
      /** Whether the strap's sessions could be read; without them the Log shows logged Workouts alone. */
      readonly strap: "read" | "unavailable"
    }
  | { readonly state: "unavailable"; readonly reason: WorkoutFailure }

/** Actionable static copy, safe for public failure evidence. */
export const workoutFailureMessage: Record<WorkoutFailure, string> = {
  "missing-configuration":
    "Connect OpenHome in Connection before opening workouts.",
  "invalid-configuration": "Check the private OpenHome setup in Connection.",
  "rejected-credential": "OpenHome rejected the API key. Check Connection.",
  unreachable:
    "OpenHome is unreachable. Open the page again when the connection returns.",
  unavailable:
    "OpenHome could not read the workouts. Open the page again to retry.",
  invalid: "OpenHome rejected the workout read. Open the page again to retry.",
  "missing-exercise":
    "An exercise in the log is no longer in the library. Open the page again to retry.",
  "not-found":
    "A workout in the log is no longer stored. Open the page again to see current workouts.",
}

const trainingDay = new Intl.DateTimeFormat("en-CA", {
  timeZone: "Europe/Copenhagen",
  year: "numeric",
  month: "2-digit",
  day: "2-digit",
})

/**
 * Name the household's current Training Day.
 *
 * @param now - The instant to place on the Copenhagen calendar.
 * @returns The `YYYY-MM-DD` Training Day.
 */
export function trainingDayOn(now: Date): string {
  return trainingDay.format(now)
}

const number = new Intl.NumberFormat("en-GB", {
  maximumFractionDigits: 2,
  useGrouping: false,
})

function formatNumber(value: number): string {
  return number.format(value)
}

/**
 * Format a hold for display and editing.
 *
 * @param seconds - The stored duration.
 * @returns Seconds under a minute (`45`), otherwise minutes and seconds (`1:30`).
 */
export function formatHold(seconds: number): string {
  if (seconds < 60) return seconds.toString()

  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`
}

/**
 * Label a stored duration in words.
 *
 * @param seconds - The stored duration, null when none was recorded.
 * @returns For example `45 s` or `1:30`.
 */
export function holdLabel(seconds: number | null): string {
  if (seconds === null) return "—"

  return seconds < 60 ? `${seconds} s` : formatHold(seconds)
}

/** A Set's glanceable figures; `load` is null for bodyweight. */
export type SetFigures = {
  readonly effort: string
  readonly load: string | null
}

/**
 * The two figures a Set is read by at a glance: its effort and its Added Weight.
 *
 * @param set - A stored Set.
 * @returns Reps (`8`) or a hold (`1:05`), and `+20` or null for bodyweight.
 */
export function setFigures(set: LoggedSet): SetFigures {
  return {
    effort:
      set.reps === null ? holdLabel(set.duration_seconds) : String(set.reps),
    load: set.weight_kg === null ? null : `+${formatNumber(set.weight_kg)}`,
  }
}

/**
 * Describe one stored Set in words, keeping bodyweight distinct from zero load.
 *
 * @param set - The stored Set.
 * @returns For example `8 reps · +20 kg · RPE 8` or `Hold 45 s · bodyweight`.
 */
export function describeSet(set: LoggedSet): string {
  const effort =
    set.reps === null
      ? `Hold ${holdLabel(set.duration_seconds)}`
      : `${set.reps} ${set.reps === 1 ? "rep" : "reps"}`

  // Axum also accepts a duration on a reps Set; show it rather than hide stored data.
  const timed =
    set.reps === null || set.duration_seconds === null
      ? undefined
      : `in ${holdLabel(set.duration_seconds)}`

  const load =
    set.weight_kg === null ? "bodyweight" : `+${formatNumber(set.weight_kg)} kg`

  return [
    timed === undefined ? effort : `${effort} ${timed}`,
    load,
    set.rpe === null ? undefined : `RPE ${set.rpe}`,
  ]
    .filter((part) => part !== undefined)
    .join(" · ")
}
