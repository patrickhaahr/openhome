import { z } from "zod"
import { formatWhole } from "./number-format"
import type { NutritionFailure } from "./nutrition"

/** The ranges the Trends view offers, in days ending today. */
export const trendRanges = [30, 90, 180, 365] as const

/** A Trends range in days. */
export type TrendRange = (typeof trendRanges)[number]

/** How the Trends view groups days: ISO weeks run Monday to Sunday, months are calendar months. */
export const aggregations = ["day", "week", "month"] as const

/** A Trends aggregation. */
export type Aggregation = (typeof aggregations)[number]

const trendRange = z.union([
  z.literal(30),
  z.literal(90),
  z.literal(180),
  z.literal(365),
])

const aggregation = z.enum(aggregations)

/** The Food page's Trends search parameters; each falls back to its default when absent. */
export const trendsSearch = z.object({
  view: z.enum(["day", "trends"]).optional().catch(undefined),
  range: trendRange.optional().catch(undefined),
  by: aggregation.optional().catch(undefined),
})

/** The Trends view's chosen range and aggregation. */
export const trendsChoice = z.object({ range: trendRange, by: aggregation })

/** A complete Trends choice. */
export type TrendsChoice = z.infer<typeof trendsChoice>

/**
 * The aggregation a range opens with: days for a month, weeks up to half a year, months for a year.
 *
 * @param range - The chosen range.
 * @returns The default aggregation for it.
 */
export function defaultAggregation(range: TrendRange): Aggregation {
  if (range === 30) return "day"

  return range === 365 ? "month" : "week"
}

/**
 * The choice a search names, with defaults filled in.
 *
 * @param search - The Food page's Trends parameters.
 * @returns The range (30 days when absent) and aggregation.
 */
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

/** One nutrient over a period: its mean share of the Recommended Intake and its Upper Limit days. */
export type NutrientCoverage = z.infer<typeof nutrientCoverage>

const rangeNutrient = nutrientCoverage.extend({
  name: z.string(),
  group: z.enum(["vitamins", "minerals", "fatty_acids"]),
  unit: z.string(),
  adequate_intake: z.boolean(),
  lasting_shortfall: z.boolean(),
})

/** One nutrient over the whole range, named, with Axum's lasting shortfall verdict. */
export type RangeNutrient = z.infer<typeof rangeNutrient>

/** The range's micronutrient coverage as Axum judges it, or why there is none. */
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

/** The range's micronutrient coverage; the dashboard only draws it. */
export type MicronutrientCoverage = z.infer<typeof micronutrientCoverage>

const summaryPeriod = z.object({
  start: z.iso.date(),
  end: z.iso.date(),
  days: z.number().int().positive(),
  partial: z.boolean(),
  logged_days: z.number().int().nonnegative(),
  missing_days: z.number().int().nonnegative(),
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

/** One day, week or month of the Nutrition Summary. */
export type SummaryPeriod = z.infer<typeof summaryPeriod>

/** Axum's Nutrition Summary over a range; the dashboard only draws it. */
export const nutritionSummary = z.object({
  from: z.iso.date(),
  to: z.iso.date(),
  aggregation,
  time_zone: z.literal("Europe/Copenhagen"),
  logged_days: z.number().int().nonnegative(),
  missing_days: z.number().int().nonnegative(),
  micronutrients: micronutrientCoverage,
  periods: z.array(summaryPeriod),
})

/** The Nutrition Summary as Axum serves it. */
export type NutritionSummary = z.infer<typeof nutritionSummary>

/** The Trends view's read: the summary, or a sanitized failure. */
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

/**
 * A period's short axis label: "3 Apr" for a day or week, "Apr" for a month.
 *
 * @param period - The period.
 * @param by - The summary's aggregation.
 * @returns The label.
 */
export function axisLabel(period: SummaryPeriod, by: Aggregation): string {
  return by === "month"
    ? shortMonth.format(noon(period.start))
    : shortDay.format(noon(period.start))
}

/**
 * A period's full name, saying when an edge week or month covers only part of itself.
 *
 * @param period - The period.
 * @param by - The summary's aggregation.
 * @returns "Thu 2 Apr", "Week of 30 Mar" or "April 2026", with "(5 days in range)" when partial.
 */
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

/**
 * A mean of per-period means, each weighted by how many days it stands for.
 *
 * @param periods - The periods.
 * @param value - The period's mean, or null.
 * @param weight - The days the mean covers.
 * @returns The weighted mean, or null when no period has a value.
 */
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

/**
 * The range's NOVA 1 share: Class 1 energy over classified energy, summed before dividing.
 *
 * @param periods - The periods.
 * @returns The share 0-1, or null when nothing classified has energy.
 */
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

/** How a period stands against a nutrient's Upper Limits. */
export type PeriodLimit = "cut_down" | "above" | "clear"

/** One nutrient's row of the coverage plate: the range figures and each period's cell. */
export type CoverageRow = RangeNutrient & {
  /** Per period, in order: the mean share of the Recommended Intake and its Upper Limit state. */
  readonly cells: ReadonlyArray<{
    readonly percent: number | null
    readonly limit: PeriodLimit
  }>
}

function periodLimit(coverage: NutrientCoverage | undefined): PeriodLimit {
  if (coverage?.seven_day_average_above_upper_limit === true) return "cut_down"

  return (coverage?.days_above_upper_limit ?? 0) > 0 ? "above" : "clear"
}

/**
 * Each nutrient of the range with its cell in every period, matched by key so Axum's order holds.
 *
 * @param nutrients - The range's nutrients.
 * @param periods - The summary's periods.
 * @returns One row per nutrient.
 */
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

/**
 * The periods a nutrient's trailing 7-day average was above an Upper Limit, by name.
 *
 * @param row - A coverage row.
 * @param names - Each period's full name.
 * @returns The names of its cut down periods, in order.
 */
export function cutDownPeriods(
  row: CoverageRow,
  names: readonly string[]
): string[] {
  return row.cells.flatMap((cell, index) =>
    cell.limit === "cut_down" ? [names[index] ?? ""] : []
  )
}

/**
 * kg per week with its sign: "−0.40 kg/wk"; "0.00 kg/wk" without a sign.
 *
 * @param value - The weekly change in kg.
 * @returns The signed weekly change.
 */
export function formatWeekly(value: number): string {
  const text = Math.abs(value).toFixed(2)

  if (Number(text) === 0) return `${text} kg/wk`

  return `${value < 0 ? "−" : "+"}${text} kg/wk`
}

/**
 * Whole kcal with grouping: "2,120 kcal".
 *
 * @param value - Energy in kcal.
 * @returns The formatted energy.
 */
export function formatKcal(value: number): string {
  return `${formatWhole(value)} kcal`
}

/** Actionable copy for a failed Trends read, without upstream error bodies. */
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
