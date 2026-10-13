import { z } from "zod"
import type { Exercise, WorkoutFailure } from "./workout"

const id = z.number().int()

/** Most Exercises Axum returns in one combined read. */
export const maxCompared = 24

/** Selectable chart periods, each ending on today's Training Day. */
export const progressPeriods = {
  "4w": { days: 28, label: "4 weeks" },
  "3m": { days: 91, label: "3 months" },
  "12m": { days: 365, label: "12 months" },
} as const

/** A chart period key. */
export type ProgressPeriod = keyof typeof progressPeriods

/** Period and explicitly compared Exercises; without `compare` Axum picks recent ones. */
export const progressSearch = z.object({
  period: z.enum(["4w", "3m", "12m"]).optional().catch(undefined),
  compare: id
    .positive()
    .array()
    .min(1)
    .max(maxCompared)
    .optional()
    .catch(undefined),
})

/** Parsed progress search parameters. */
export type ProgressSearch = z.infer<typeof progressSearch>

const exerciseRef = z.object({
  id,
  name: z.string(),
  category: z.enum(["calisthenics", "gym"]),
})

const sample = z.object({
  date: z.iso.date(),
  value: z.number().int(),
  index: z.number(),
})

/** One observed day: the raw best value and its index against the baseline of 100. */
export type ProgressSample = z.infer<typeof sample>

const chartedLine = z.object({
  exercise: exerciseRef,
  status: z.literal("charted"),
  // Axum also sends `unit`; it follows from `metric`, so it is not kept.
  metric: z.enum(["repetitions", "hold_seconds"]),
  baseline: z.object({ date: z.iso.date(), value: z.number().int() }),
  samples: sample.array().min(1),
})

/** An Exercise Axum indexed: bodyweight reps or Timed Hold seconds, observed days only. */
export type ChartedLine = z.infer<typeof chartedLine>

const progressLine = z.discriminatedUnion("status", [
  chartedLine,
  z.object({
    exercise: exerciseRef,
    status: z.literal("unsupported"),
    reason: z.enum(["added_weight", "mixed_set_kinds", "no_positive_value"]),
  }),
  z.object({ exercise: exerciseRef, status: z.literal("not_logged_in_range") }),
  z.object({ exercise: exerciseRef, status: z.literal("never_logged") }),
])

/** One requested Exercise, charted or with the reason it has no line. */
export type ProgressLine = z.infer<typeof progressLine>

/** The combined Progress Lines Axum serves for one period. */
export const combinedProgress = z.object({
  from: z.iso.date(),
  to: z.iso.date(),
  logged_exercise_count: z.number().int(),
  exercises: progressLine.array(),
})

/** A complete combined progress read. */
export type CombinedProgress = z.infer<typeof combinedProgress>

/** A complete chart read, or an outcome that must never look like an empty chart. */
export type ProgressState =
  | {
      readonly state: "available"
      readonly period: ProgressPeriod
      readonly progress: CombinedProgress
      readonly library: ReadonlyArray<Exercise>
      /** The explicitly compared Exercise ids still in the library; empty means most recent. */
      readonly compared: ReadonlyArray<number>
    }
  | { readonly state: "unavailable"; readonly reason: WorkoutFailure }

/** Static, actionable copy for an unavailable chart. */
export const progressFailureMessage: Record<WorkoutFailure, string> = {
  "missing-configuration":
    "Connect OpenHome in Connection before opening progress.",
  "invalid-configuration": "Check the private OpenHome setup in Connection.",
  "rejected-credential": "OpenHome rejected the API key. Check Connection.",
  unreachable: "OpenHome is unreachable. Refresh when the connection returns.",
  unavailable: "OpenHome could not read progress. Refresh to try again.",
  invalid: "OpenHome rejected the progress request. Choose another period.",
  "missing-exercise":
    "A compared exercise is no longer in the library. Refresh to update the chart.",
  "not-found":
    "A compared exercise is no longer in the library. Refresh to update the chart.",
}

/** Why an Exercise has no line, in the household's words. */
export function missingLineReason(
  line: Exclude<ProgressLine, ChartedLine>
): string {
  switch (line.status) {
    case "never_logged":
      return "Never logged"
    case "not_logged_in_range":
      return "Not logged in this period"
    case "unsupported":
      switch (line.reason) {
        case "added_weight":
          return "Has sets with added weight; weighted progress is not charted yet"
        case "mixed_set_kinds":
          return "Mixes reps and timed holds, so no single measure fits"
        case "no_positive_value":
          return "Every logged value is zero, so there is no baseline"
      }
  }
}

const metricLabels: Record<ChartedLine["metric"], string> = {
  repetitions: "Best set reps",
  hold_seconds: "Longest hold",
}

/** The measure a line follows. */
export function metricLabel(line: ChartedLine): string {
  return metricLabels[line.metric]
}

/** A raw value with its unit, holds as minutes and seconds like the workout log. */
export function rawValue(line: ChartedLine, value: number): string {
  if (line.metric === "repetitions")
    return `${value} ${value === 1 ? "rep" : "reps"}`

  return value >= 60
    ? `${Math.floor(value / 60)}:${String(value % 60).padStart(2, "0")} min`
    : `${value} s`
}

/** An index rounded for display; 100 is the baseline. */
export function indexValue(index: number): string {
  return String(Math.round(index))
}

const dayMs = 86_400_000

function dayNumber(day: string): number {
  return Date.parse(`${day}T00:00:00Z`) / dayMs
}

/** Inclusive `YYYY-MM-DD` bounds Axum reads progress for. */
export type PeriodBounds = { readonly from: string; readonly to: string }

/** Axum's combined progress query; without ids Axum picks recently logged Exercises. */
export type ProgressQuery = PeriodBounds & { readonly exercise_ids?: string }

/**
 * The inclusive calendar period ending on `today`.
 *
 * @param period - The chosen period.
 * @param today - Today's `YYYY-MM-DD` Training Day.
 * @returns `YYYY-MM-DD` bounds for Axum.
 */
export function periodBounds({
  period,
  today,
}: {
  readonly period: ProgressPeriod
  readonly today: string
}): PeriodBounds {
  const from = new Date(
    (dayNumber(today) - progressPeriods[period].days + 1) * dayMs
  )

  return { from: from.toISOString().slice(0, 10), to: today }
}

/** Chart drawing area in SVG user units. */
export const plot = {
  width: 720,
  height: 300,
  left: 48,
  right: 12,
  top: 12,
  bottom: 28,
} as const

/** Consecutive samples further apart than this are joined with a dashed line. */
export const gapDays = 14

/** Scales for the visible lines; the y range always includes the baseline of 100. */
export type ChartScale = {
  readonly x: (day: string) => number
  readonly y: (index: number) => number
  readonly ticks: ReadonlyArray<number>
}

const tickSteps = [5, 10, 20, 25, 50, 100, 200, 250, 500, 1000]

/**
 * Fit the period and visible indexes into the plot, with at most six round y ticks.
 *
 * @param period - The period's first and last days.
 * @param lines - The lines currently shown.
 * @returns Scales and ticks for drawing.
 */
export function chartScale({
  from,
  to,
  lines,
}: PeriodBounds & {
  readonly lines: ReadonlyArray<ChartedLine>
}): ChartScale {
  const indexes = lines.flatMap((line) =>
    line.samples.map((sample) => sample.index)
  )

  const low = Math.min(100, ...indexes)

  const high = Math.max(100, ...indexes)

  const step =
    tickSteps.find((candidate) => (high - low) / candidate <= 5) ?? 2000

  const bottom = Math.floor(low / step) * step

  const top = Math.max(Math.ceil(high / step) * step, bottom + step)

  const ticks: number[] = []

  for (let tick = bottom; tick <= top; tick += step) ticks.push(tick)

  const first = dayNumber(from)
  const span = Math.max(1, dayNumber(to) - first)
  const width = plot.width - plot.left - plot.right
  const height = plot.height - plot.top - plot.bottom

  return {
    x: (day) => plot.left + ((dayNumber(day) - first) / span) * width,
    y: (index) => plot.top + ((top - index) / (top - bottom)) * height,
    ticks,
  }
}

/** Whether two observed days are far enough apart to draw the join as a gap. */
export function isGap(earlier: string, later: string): boolean {
  return dayNumber(later) - dayNumber(earlier) > gapDays
}

const shortDate = new Intl.DateTimeFormat("en-GB", {
  day: "numeric",
  month: "short",
  timeZone: "UTC",
})

/** A `YYYY-MM-DD` day as "1 Sep". */
export function shortDay(day: string): string {
  return shortDate.format(new Date(`${day}T00:00:00Z`))
}
