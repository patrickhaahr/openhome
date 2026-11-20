import { z } from "zod"
import { formatWhole } from "./number-format"
import type { NutritionFailure } from "./nutrition"

export const trendRanges = [30, 90, 180, 365] as const

export type TrendRange = (typeof trendRanges)[number]

export const aggregations = ["day", "week", "month"] as const

export type Aggregation = (typeof aggregations)[number]

const trendRange = z.union([
  z.literal(30),
  z.literal(90),
  z.literal(180),
  z.literal(365),
])

const aggregation = z.enum(aggregations)

export const trendsSearch = z.object({
  view: z.enum(["day", "trends"]).optional().catch(undefined),
  range: trendRange.optional().catch(undefined),
  by: aggregation.optional().catch(undefined),
})

export const trendsChoice = z.object({ range: trendRange, by: aggregation })

export type TrendsChoice = z.infer<typeof trendsChoice>

export function defaultAggregation(range: TrendRange): Aggregation {
  if (range === 30) return "day"

  return range === 365 ? "month" : "week"
}

export function trendsChoiceOf(
  search: z.infer<typeof trendsSearch>
): TrendsChoice {
  const range = search.range ?? 30

  return { range, by: search.by ?? defaultAggregation(range) }
}

const kcal = z.number().nullable()

const nutrientCoverage = z.object({
  key: z.string(),
  percent_of_recommended: z.number().nonnegative().nullable(),
  averaged_days: z.number().int().nonnegative(),
  days_above_upper_limit: z.number().int().nonnegative().nullable(),
  seven_day_average_above_upper_limit: z.boolean().nullable(),
})

export type NutrientCoverage = z.infer<typeof nutrientCoverage>

const rangeNutrient = nutrientCoverage.extend({
  name: z.string(),
  group: z.enum(["vitamins", "minerals", "fatty_acids"]),
  unit: z.string(),
  adequate_intake: z.boolean(),
  lasting_shortfall: z.boolean(),
})

export type RangeNutrient = z.infer<typeof rangeNutrient>

const micronutrientCoverage = z.discriminatedUnion("state", [
  z.object({
    state: z.literal("available"),
    logged_days: z.number().int().nonnegative(),
    shortfall: z.object({
      below_percent: z.number().positive(),
      min_logged_days: z.number().int().positive(),
    }),
    nutrients: z.array(rangeNutrient),
    sources: z.array(z.object({ name: z.string(), citation: z.string() })),
  }),
  z.object({
    state: z.literal("unavailable"),
    missing: z.array(z.enum(["sex", "birth_date"])),
    reason: z.string(),
  }),
])

export type MicronutrientCoverage = z.infer<typeof micronutrientCoverage>

const credited = z.object({ credit: z.string(), licence: z.string() })

const summaryPeriod = z.object({
  start: z.iso.date(),
  end: z.iso.date(),
  days: z.number().int().positive(),
  partial: z.boolean(),
  logged_days: z.number().int().nonnegative(),
  missing_days: z.number().int().nonnegative(),
  intake_days: z.number().int().nonnegative(),
  known_days: z.object({
    energy: z.number().int().nonnegative(),
    protein: z.number().int().nonnegative(),
    carbohydrate: z.number().int().nonnegative(),
    fat: z.number().int().nonnegative(),
    alcohol: z.number().int().nonnegative(),
  }),
  energy: z.object({
    eaten_kcal: kcal,
    calorie_target_kcal: kcal,
    estimated_expenditure_kcal: kcal,
  }),
  protein: z.object({
    eaten_g: z.number().nullable(),
    target_g: z.number().nullable(),
  }),
  macro_energy: z.object({
    protein_kcal: kcal,
    carbohydrate_kcal: kcal,
    fat_kcal: kcal,
    alcohol_kcal: kcal,
  }),
  nova: z.object({
    nova_1_share: z.number().min(0).max(1).nullable(),
    nova_1_kcal: z.number().nonnegative(),
    classified_kcal: z.number().nonnegative(),
    unclassified_portions: z.number().int().nonnegative(),
  }),
  weight: z.object({
    trend_weight_kg: z.number().nullable(),
    scale_weight_kg: z.number().nullable(),
    weigh_ins: z.number().int().nonnegative(),
  }),
  weight_change: z.object({
    kg_per_week: z.number().nullable(),
    goal_kg_per_week: z.number(),
  }),
  micronutrients: z
    .object({
      logged_days: z.number().int().nonnegative(),
      nutrients: z.array(nutrientCoverage),
    })
    .nullable(),
})

export type SummaryPeriod = z.infer<typeof summaryPeriod>

export const nutritionSummary = z.object({
  from: z.iso.date(),
  to: z.iso.date(),
  aggregation,
  time_zone: z.literal("Europe/Copenhagen"),
  logged_days: z.number().int().nonnegative(),
  missing_days: z.number().int().nonnegative(),
  micronutrients: micronutrientCoverage,
  sources: z.array(credited),
  label_source: credited.nullable(),
  periods: z.array(summaryPeriod),
})

export type NutritionSummary = z.infer<typeof nutritionSummary>

export type NutritionSummaryState = TrendsChoice & {
  readonly today: string
} & (
    | { readonly state: "available"; readonly data: NutritionSummary }
    | { readonly state: "unavailable"; readonly reason: NutritionFailure }
  )

const shortDay = new Intl.DateTimeFormat("en-GB", {
  timeZone: "UTC",
  day: "numeric",
  month: "short",
})

const weekdayDay = new Intl.DateTimeFormat("en-GB", {
  timeZone: "UTC",
  weekday: "short",
  day: "numeric",
  month: "short",
})

const monthYear = new Intl.DateTimeFormat("en-GB", {
  timeZone: "UTC",
  month: "long",
  year: "numeric",
})

const shortMonth = new Intl.DateTimeFormat("en-GB", {
  timeZone: "UTC",
  month: "short",
})

function noon(day: string): Date {
  return new Date(`${day}T12:00:00Z`)
}

export function axisLabel(period: SummaryPeriod, by: Aggregation): string {
  return by === "month"
    ? shortMonth.format(noon(period.start))
    : shortDay.format(noon(period.start))
}

export function periodName(period: SummaryPeriod, by: Aggregation): string {
  const name = calendarName(period.start, by)
  const days = period.days === 1 ? "1 day" : `${period.days} days`

  return period.partial ? `${name} (${days} in range)` : name
}

function calendarName(start: string, by: Aggregation): string {
  switch (by) {
    case "day":
      return weekdayDay.format(noon(start))
    case "week":
      return `Week of ${shortDay.format(noon(start))}`
    case "month":
      return monthYear.format(noon(start))
  }
}

export function weightedMean(
  periods: readonly SummaryPeriod[],
  value: (period: SummaryPeriod) => number | null,
  weight: (period: SummaryPeriod) => number
): number | null {
  let sum = 0
  let days = 0

  for (const period of periods) {
    const mean = value(period)
    const count = weight(period)

    if (mean === null || count === 0) continue
    sum += mean * count
    days += count
  }

  return days === 0 ? null : sum / days
}

export function novaShareOf(periods: readonly SummaryPeriod[]): number | null {
  const classified = periods.reduce(
    (sum, period) => sum + period.nova.classified_kcal,
    0
  )

  const unprocessed = periods.reduce(
    (sum, period) => sum + period.nova.nova_1_kcal,
    0
  )

  return classified > 0 ? unprocessed / classified : null
}

export type PeriodLimit = "cut_down" | "above" | "clear"

export type CoverageRow = RangeNutrient & {
  readonly cells: ReadonlyArray<{
    readonly percent: number | null
    readonly limit: PeriodLimit
  }>
}

function periodLimit(coverage: NutrientCoverage | undefined): PeriodLimit {
  if (coverage?.seven_day_average_above_upper_limit === true) return "cut_down"

  return (coverage?.days_above_upper_limit ?? 0) > 0 ? "above" : "clear"
}

export function coverageRows(
  nutrients: readonly RangeNutrient[],
  periods: readonly SummaryPeriod[]
): CoverageRow[] {
  const byPeriod = periods.map(
    (period) =>
      new Map(
        (period.micronutrients?.nutrients ?? []).map((nutrient) => [
          nutrient.key,
          nutrient,
        ])
      )
  )

  return nutrients.map((nutrient) => ({
    ...nutrient,
    cells: byPeriod.map((coverage) => {
      const cell = coverage.get(nutrient.key)

      return {
        percent: cell?.percent_of_recommended ?? null,
        limit: periodLimit(cell),
      }
    }),
  }))
}

export function cutDownPeriods(
  row: CoverageRow,
  names: readonly string[]
): string[] {
  return row.cells.flatMap((cell, index) =>
    cell.limit === "cut_down" ? [names[index] ?? ""] : []
  )
}

export function formatWeekly(value: number): string {
  const text = Math.abs(value).toFixed(2)

  if (Number(text) === 0) return `${text} kg/wk`

  return `${value < 0 ? "−" : "+"}${text} kg/wk`
}

export function formatKcal(value: number): string {
  return `${formatWhole(value)} kcal`
}

export const trendsFailureMessage: Record<NutritionFailure, string> = {
  "missing-configuration":
    "Connect OpenHome in Connection before opening Food.",
  "invalid-configuration": "Check the private OpenHome setup in Connection.",
  "rejected-credential": "OpenHome rejected the API key. Check Connection.",
  unreachable: "OpenHome did not answer. Reopen the page to try again.",
  unavailable:
    "OpenHome could not read these trends. Reopen the page to try again.",
  invalid: "These trends cannot be shown. Choose another range.",
}
