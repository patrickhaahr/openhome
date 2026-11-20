import { z } from "zod"
import type { NoopSummary, NoopWorkoutDetail } from "./training"

const id = z.number().int()

const exerciseCategory = z.enum(["calisthenics", "gym"])

export const exercise = z.object({
  id,
  name: z.string(),
  category: exerciseCategory,
  muscle_group: z.string().nullable(),
  equipment: z.string().nullable(),
})

export type Exercise = z.infer<typeof exercise>

export const workoutSummary = z.object({
  id,
  date: z.string(),
  name: z.string().nullable(),
})

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

export type LoggedSet = z.infer<typeof loggedSet>

const workoutEntry = z.object({
  id,
  order_index: z.number().int(),
  notes: z.string().nullable(),
  exercise: z.object({ id, name: z.string(), category: exerciseCategory }),
  sets: loggedSet.array(),
})

export type WorkoutEntry = z.infer<typeof workoutEntry>

export const workoutDetail = z.object({
  id,
  date: z.string(),
  name: z.string().nullable(),
  notes: z.string().nullable(),
  exercises: workoutEntry.array(),
})

export type WorkoutDetail = z.infer<typeof workoutDetail>

export const historyStep = 20

export const maxHistory = 480

export const workoutSearch = z.object({
  shown: z
    .number()
    .int()
    .min(historyStep)
    .max(maxHistory)
    .optional()
    .catch(undefined),
})

export type WorkoutSearch = z.infer<typeof workoutSearch>

export type WorkoutFailure =
  | "missing-configuration"
  | "invalid-configuration"
  | "rejected-credential"
  | "unreachable"
  | "unavailable"
  | "invalid"
  | "missing-exercise"
  | "not-found"

export type Strap = {
  readonly summary: NoopSummary
  readonly detail: NoopWorkoutDetail | undefined
}

export type LogSession = {
  readonly key: string
  readonly day: string
  readonly workout: WorkoutDetail | undefined
  readonly strap: Strap | undefined
}

export type WorkoutState =
  | {
      readonly state: "available"
      readonly exercises: Exercise[]
      readonly sessions: LogSession[]
      readonly hasOlder: boolean
      readonly strap: "read" | "unavailable"
    }
  | { readonly state: "unavailable"; readonly reason: WorkoutFailure }

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

export function formatHold(seconds: number): string {
  if (seconds < 60) return seconds.toString()

  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`
}

export function holdLabel(seconds: number | null): string {
  if (seconds === null) return "—"

  return seconds < 60 ? `${seconds} s` : formatHold(seconds)
}

export type SetFigures = {
  readonly effort: string
  readonly load: string | null
}

export function setFigures(set: LoggedSet): SetFigures {
  return {
    effort:
      set.reps === null ? holdLabel(set.duration_seconds) : String(set.reps),
    load: set.weight_kg === null ? null : `+${formatNumber(set.weight_kg)}`,
  }
}

export function describeSet(set: LoggedSet): string {
  const effort =
    set.reps === null
      ? `Hold ${holdLabel(set.duration_seconds)}`
      : `${set.reps} ${set.reps === 1 ? "rep" : "reps"}`

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
