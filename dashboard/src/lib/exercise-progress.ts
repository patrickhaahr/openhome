import { z } from "zod"
import type { WorkoutFailure } from "./workout"

const id = z.number().int()

export const maxCompared = 24

export const progressPeriods = {
  "4w": { days: 28, label: "4 weeks" },
  "3m": { days: 91, label: "3 months" },
  "12m": { days: 365, label: "12 months" },
} as const

export type ProgressPeriod = keyof typeof progressPeriods

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

export type ProgressSearch = z.infer<typeof progressSearch>

const exerciseRef = z.object({
  id,
  name: z.string(),
  category: z.enum(["calisthenics", "gym"]),
})

const sample = z.object({
  date: z.iso.date(),
  value: z.number(),
  index: z.number(),
})

export type ProgressSample = z.infer<typeof sample>

const chartedLine = z.object({
  exercise: exerciseRef,
  status: z.literal("charted"),
  metric: z.enum(["repetitions", "hold_seconds", "estimated_1rm"]),
  baseline: z.object({ date: z.iso.date(), value: z.number() }),
  samples: sample.array().min(1),
  omitted_dates: z.iso.date().array(),
})

export type ChartedLine = z.infer<typeof chartedLine>

const progressLine = z.discriminatedUnion("status", [
  chartedLine,
  z.object({
    exercise: exerciseRef,
    status: z.literal("unsupported"),
    reason: z.enum([
      "weighted_holds",
      "no_valid_estimate",
      "mixed_set_kinds",
      "no_positive_value",
    ]),
  }),
  z.object({ exercise: exerciseRef, status: z.literal("not_logged_in_range") }),
  z.object({ exercise: exerciseRef, status: z.literal("never_logged") }),
])

export type ProgressLine = z.infer<typeof progressLine>

export const combinedProgress = z.object({
  from: z.iso.date(),
  to: z.iso.date(),
  logged_exercise_count: z.number().int(),
  exercises: progressLine.array(),
})

export type CombinedProgress = z.infer<typeof combinedProgress>

export function byLatestTrained(
  lines: ReadonlyArray<ProgressLine>
): ProgressLine[] {
  const latest = (line: ProgressLine): string =>
    line.status === "charted" ? (line.samples.at(-1)?.date ?? "") : ""

  return [...lines].sort(
    (a, b) =>
      latest(b).localeCompare(latest(a)) ||
      a.exercise.name.localeCompare(b.exercise.name)
  )
}

export type ProgressState =
  | {
      readonly state: "available"
      readonly period: ProgressPeriod
      readonly progress: CombinedProgress
      readonly compared: ReadonlyArray<number>
    }
  | { readonly state: "unavailable"; readonly reason: WorkoutFailure }

export const progressFailureMessage: Record<WorkoutFailure, string> = {
  "missing-configuration":
    "Connect OpenHome in Connection before opening progress.",
  "invalid-configuration": "Check the private OpenHome setup in Connection.",
  "rejected-credential": "OpenHome rejected the API key. Check Connection.",
  unreachable: "OpenHome is unreachable. Reload when the connection returns.",
  unavailable: "OpenHome could not read progress. Reload to try again.",
  invalid: "OpenHome rejected the progress request. Choose another period.",
  "missing-exercise":
    "A compared exercise is no longer in the library. Reload to update the chart.",
  "not-found":
    "A compared exercise is no longer in the library. Reload to update the chart.",
}

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
        case "weighted_holds":
          return "Weighted holds have no supported estimate"
        case "no_valid_estimate":
          return "No valid estimate: needs a positive added load at 1–10 reps"
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
  estimated_1rm: "Estimated 1RM",
}

export const metricPolicy: Record<ChartedLine["metric"], string> = {
  repetitions: "Best unweighted set on each logged day, in repetitions.",
  hold_seconds: "Longest unweighted hold on each logged day, in seconds.",
  estimated_1rm:
    "Best Epley estimate from positive added loads at 1–10 reps each day; one rep uses the lifted load. Added load only: body weight is excluded, so this is not total strength for bodyweight movements. Unweighted sets and other rep ranges are excluded. Estimates assume a near-maximal set; they are not tested maximums.",
}

const kilograms = new Intl.NumberFormat("en-GB", { maximumFractionDigits: 1 })

export function metricLabel(line: ChartedLine): string {
  return metricLabels[line.metric]
}

export function rawValue(line: ChartedLine, value: number): string {
  if (line.metric === "estimated_1rm") return `${kilograms.format(value)} kg`

  if (line.metric === "repetitions")
    return `${value} ${value === 1 ? "rep" : "reps"}`

  return value >= 60
    ? `${Math.floor(value / 60)}:${String(value % 60).padStart(2, "0")} min`
    : `${value} s`
}

export function indexValue(index: number): string {
  return String(Math.round(index))
}

const dayMs = 86_400_000

function dayNumber(day: string): number {
  return Date.parse(`${day}T00:00:00Z`) / dayMs
}

export type PeriodBounds = { readonly from: string; readonly to: string }

export type ProgressQuery = PeriodBounds & { readonly exercise_ids?: string }

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

export const plot = {
  width: 960,
  height: 290,
  left: 48,
  right: 12,
  top: 12,
  bottom: 28,
} as const

export const gapDays = 14

export type ChartScale = {
  readonly x: (day: string) => number
  readonly y: (index: number) => number
  readonly ticks: ReadonlyArray<number>
}

const tickSteps = [5, 10, 20, 25, 50, 100, 200, 250, 500, 1000]

export function chartScale({
  from,
  to,
  lines,
  size = plot,
}: PeriodBounds & {
  readonly lines: ReadonlyArray<ChartedLine>
  readonly size?: { readonly width: number; readonly height: number }
}): ChartScale {
  const indexes = lines.flatMap((line) =>
    line.samples.map((sample) => sample.index)
  )

  const low = Math.min(100, ...indexes)

  const high = Math.max(100, ...indexes)

  const targetStep = (high - low) / 4
  const magnitude = 10 ** Math.floor(Math.log10(targetStep || 1))

  const step =
    tickSteps.find((candidate) => targetStep <= candidate) ??
    (targetStep <= magnitude * 2
      ? magnitude * 2
      : targetStep <= magnitude * 5
        ? magnitude * 5
        : magnitude * 10)

  const bottom = Math.floor(low / step) * step

  const roundedTop = Math.ceil(high / step) * step

  const top = Math.max(
    Number.isFinite(roundedTop) ? roundedTop : high,
    bottom + step
  )

  const ticks: number[] = []

  for (let position = 0; position < 6; position++) {
    const tick = bottom + position * step

    if (tick > top) break
    ticks.push(tick)
  }

  const first = dayNumber(from)
  const span = Math.max(1, dayNumber(to) - first)
  const width = size.width - plot.left - plot.right
  const height = size.height - plot.top - plot.bottom

  return {
    x: (day) => plot.left + ((dayNumber(day) - first) / span) * width,
    y: (index) => plot.top + ((top - index) / (top - bottom)) * height,
    ticks,
  }
}

export function isGap(earlier: string, later: string): boolean {
  return dayNumber(later) - dayNumber(earlier) > gapDays
}

const shortDate = new Intl.DateTimeFormat("en-GB", {
  day: "numeric",
  month: "short",
  timeZone: "UTC",
})

export function shortDay(day: string): string {
  return shortDate.format(new Date(`${day}T00:00:00Z`))
}
