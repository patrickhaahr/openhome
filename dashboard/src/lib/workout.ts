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

const setInput = z.object({
  set_number: z.number().int(),
  reps: z.number().int().nullable(),
  weight_kg: z.number().nullable(),
  duration_seconds: z.number().int().nullable(),
  rpe: z.number().int().nullable(),
  notes: z.string().nullable(),
})

/** A complete Workout write; Axum owns Set validation, Exercise existence and atomicity. */
export const workoutInput = z.object({
  date: z.string(),
  name: z.string().nullable(),
  notes: z.string().nullable(),
  exercises: z
    .object({
      exercise_id: id,
      order_index: z.number().int(),
      notes: z.string().nullable(),
      sets: setInput.array(),
    })
    .array(),
})

/** The body Axum stores for a new or replaced Workout. */
export type WorkoutInput = z.infer<typeof workoutInput>

/** The web client only edits stored Workouts, replacing all entries in one transaction. */
export const workoutCommand = workoutInput.extend({
  kind: z.literal("replace"),
  workout_id: id,
})

/** A full edit of one stored Workout. */
export type WorkoutCommand = z.infer<typeof workoutCommand>

/** Only a parsed Axum receipt confirms persistence. */
export type WorkoutMutation =
  | { readonly state: "saved"; readonly workoutId: number }
  | { readonly state: "failed"; readonly reason: WorkoutFailure }

/** Actionable static copy, safe for public failure evidence. */
export const workoutFailureMessage: Record<WorkoutFailure, string> = {
  "missing-configuration":
    "Connect OpenHome in Connection before opening workouts.",
  "invalid-configuration": "Check the private OpenHome setup in Connection.",
  "rejected-credential": "OpenHome rejected the API key. Check Connection.",
  unreachable:
    "The response was lost. Reload to check whether the change saved before trying again.",
  unavailable:
    "OpenHome could not confirm the result. Reload before trying again.",
  invalid:
    "OpenHome rejected the workout. Check the date, each set’s reps or hold time, and RPE from 1 to 10.",
  "missing-exercise":
    "An exercise in this workout is no longer in the library. Remove it or choose another, then save again.",
  "not-found":
    "This workout is no longer stored. Reload to see current workouts.",
}

/** How a Set is performed: repetitions, or a duration-only Timed Hold. */
export type SetKind = "reps" | "hold"

/** Editable text for one Set; stored notes and a reps Set's stored duration ride along unchanged. */
export type SetDraft = {
  readonly key: string
  readonly kind: SetKind
  readonly reps: string
  readonly seconds: string
  readonly weight: string
  readonly rpe: string
  readonly notes: string | null
  readonly heldSeconds: number | null
}

/** Editable entry for one Exercise in a Workout. */
export type EntryDraft = {
  readonly key: string
  readonly exercise: WorkoutEntry["exercise"]
  readonly notes: string
  readonly sets: ReadonlyArray<SetDraft>
}

/** Unsaved Workout fields as typed on the display. */
export type WorkoutDraft = {
  readonly date: string
  readonly name: string
  readonly notes: string
  readonly entries: ReadonlyArray<EntryDraft>
}

/** The field a draft problem belongs to, so focus and highlighting land on it. */
export type DraftField = "date" | "reps" | "hold" | "weight" | "rpe"

/** Problems keyed by the draft Set key, or `date` for the Workout date. */
export type DraftProblems = ReadonlyMap<
  string,
  { readonly field: DraftField; readonly message: string }
>

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

/**
 * Create a blank Set, repeating the previous Set's values when there is one.
 *
 * @param key - A stable React key for the new Set.
 * @param previous - The Set to repeat; its notes and stored duration stay with the original.
 * @returns An editable Set.
 */
export function nextSet(key: string, previous: SetDraft | undefined): SetDraft {
  return previous === undefined
    ? {
        key,
        kind: "reps",
        reps: "",
        seconds: "",
        weight: "",
        rpe: "",
        notes: null,
        heldSeconds: null,
      }
    : { ...previous, key, notes: null, heldSeconds: null }
}

/**
 * Open a stored Workout for editing without losing fields the form does not show.
 *
 * @param workout - The stored Workout.
 * @param key - Supplies stable React keys for entries and Sets.
 * @returns An editable draft of the same Workout.
 */
export function draftFromWorkout(
  workout: WorkoutDetail,
  key: () => string
): WorkoutDraft {
  return {
    date: workout.date,
    name: workout.name ?? "",
    notes: workout.notes ?? "",
    entries: workout.exercises.map((entry) => ({
      key: key(),
      exercise: entry.exercise,
      notes: entry.notes ?? "",
      sets: entry.sets.map((set) => ({
        key: key(),
        kind: set.reps === null ? "hold" : "reps",
        reps: set.reps?.toString() ?? "",
        seconds:
          set.duration_seconds === null ? "" : formatHold(set.duration_seconds),
        // Exact text, so an unrelated edit never rounds a stored weight.
        weight: set.weight_kg === null ? "" : String(set.weight_kg),
        rpe: set.rpe?.toString() ?? "",
        notes: set.notes,
        heldSeconds: set.reps === null ? null : set.duration_seconds,
      })),
    })),
  }
}

function optionalText(value: string): string | null {
  const trimmed = value.trim()

  return trimmed === "" ? null : trimmed
}

function positiveWhole(value: string): number | undefined {
  const trimmed = value.trim()

  if (!/^\d+$/.test(trimmed)) return undefined

  const parsed = Number(trimmed)

  return parsed > 0 && Number.isSafeInteger(parsed) ? parsed : undefined
}

/**
 * Read a hold time typed as seconds (`45`) or minutes and seconds (`1:30`).
 *
 * @param value - The typed hold time.
 * @returns Whole seconds above zero, or undefined when the text is not a hold time.
 */
export function parseHold(value: string): number | undefined {
  const parts = /^(\d+):([0-5]\d)$/.exec(value.trim())

  if (parts === null) return positiveWhole(value)

  const seconds = Number(parts[1]) * 60 + Number(parts[2])

  return seconds > 0 ? seconds : undefined
}

function addedWeight(value: string): number | null | undefined {
  const trimmed = value.trim().replace(",", ".")

  if (trimmed === "") return null

  if (!/^\d+(\.\d+)?$/.test(trimmed)) return undefined

  const parsed = Number(trimmed)

  return parsed > 0 ? parsed : undefined
}

/**
 * Parse a draft into the write Axum stores, reporting each unusable field.
 *
 * An empty Added Weight means bodyweight and is sent as null, never zero.
 *
 * @param draft - The typed Workout.
 * @returns The Workout write, or the problems to show beside their fields.
 */
export function parseDraft(
  draft: WorkoutDraft
):
  | { readonly state: "parsed"; readonly input: WorkoutInput }
  | { readonly state: "problems"; readonly problems: DraftProblems } {
  const problems = new Map<
    string,
    { readonly field: DraftField; readonly message: string }
  >()

  if (!/^\d{4}-\d{2}-\d{2}$/.test(draft.date))
    problems.set("date", {
      field: "date",
      message: "Choose the training date.",
    })

  const exercises = draft.entries.map((entry, entryIndex) => ({
    exercise_id: entry.exercise.id,
    order_index: entryIndex,
    notes: optionalText(entry.notes),
    sets: entry.sets.map((set, setIndex) => {
      const reps = set.kind === "reps" ? positiveWhole(set.reps) : null

      const duration =
        set.kind === "hold" ? parseHold(set.seconds) : set.heldSeconds

      const weight = addedWeight(set.weight)
      const rpe = set.rpe === "" ? null : positiveWhole(set.rpe)

      if (reps === undefined)
        problems.set(set.key, {
          field: "reps",
          message: "Enter whole reps above zero.",
        })
      else if (duration === undefined)
        problems.set(set.key, {
          field: "hold",
          message: "Enter the hold as seconds or minutes:seconds.",
        })
      else if (weight === undefined)
        problems.set(set.key, {
          field: "weight",
          message:
            "Enter added weight in kg above zero, or leave it empty for bodyweight.",
        })
      else if (rpe !== null && (rpe === undefined || rpe > 10))
        problems.set(set.key, {
          field: "rpe",
          message: "Choose RPE from 1 to 10.",
        })

      return {
        set_number: setIndex + 1,
        reps: reps ?? null,
        weight_kg: weight ?? null,
        duration_seconds: duration ?? null,
        rpe: rpe ?? null,
        notes: set.notes,
      }
    }),
  }))

  if (problems.size > 0) return { state: "problems", problems }

  return {
    state: "parsed",
    input: {
      date: draft.date,
      name: optionalText(draft.name),
      notes: optionalText(draft.notes),
      exercises,
    },
  }
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
