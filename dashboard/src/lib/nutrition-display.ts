import type {
  DoseUnit,
  LimitShare,
  Micronutrient,
  Nutrient,
  NutrientTotal,
  NutritionDay,
  Portion,
  SupplementDose,
} from "@/lib/nutrition"

/** A Portion's display name and, for a Product, its brand. */
export type PortionName = {
  readonly name: string
  readonly brand: string | null
}

/**
 * How the Day view names a Portion: a Product by its label name and brand, a Food by its English
 * name. A Product whose label has no name falls back to the Food it was linked to.
 *
 * @param portion - A Meal Log's Portion.
 * @returns The name and, for a Product, its brand.
 */
export function portionName(portion: Portion): PortionName {
  return {
    name: portion.product?.name ?? portion.name_en,
    brand: portion.product?.brand ?? null,
  }
}

/**
 * Whether any Portion of the day was logged as a Product, whose label values Open Food Facts
 * supplied and must be credited.
 *
 * @param day - The Nutrition Day.
 * @returns True when a Product Portion is shown.
 */
export function hasProducts(day: NutritionDay): boolean {
  return day.meal_logs.some((meal) =>
    meal.portions.some((portion) => portion.product !== null)
  )
}

/**
 * How the Day view explains its NOVA 1 share: the classified energy it is a share of and the
 * Portions it leaves out, or that nothing is classified yet.
 *
 * @param nova - The day's NOVA 1 share and the energy behind it.
 * @returns One line beneath the share.
 */
export function novaDetail(nova: NutritionDay["nova"]): string {
  const count = nova.unclassified_portions

  const unclassified =
    count === 0
      ? ""
      : ` · ${count} unclassified ${count === 1 ? "portion" : "portions"} left out`

  if (nova.nova_1_share === null) {
    return count === 0
      ? "Nothing classified yet"
      : `Nothing classified yet${unclassified}`
  }

  return `Unprocessed or minimally processed, of ${formatAmount(nova.classified_kcal)} classified kcal${unclassified}`
}

const doseUnitNames: Record<DoseUnit, readonly [string, string]> = {
  tablet: ["tablet", "tablets"],
  capsule: ["capsule", "capsules"],
  scoop: ["scoop", "scoops"],
  ml: ["ml", "ml"],
}

/**
 * How many doses were taken, in the Supplement's dose unit: "1 capsule", "2 tablets", "5 ml".
 *
 * @param doses - The doses taken.
 * @param unit - The Supplement's dose unit.
 * @returns The count with its unit.
 */
export function doseCount(doses: number, unit: DoseUnit): string {
  const [one, many] = doseUnitNames[unit]

  return `${formatAmount(doses)} ${doses === 1 ? one : many}`
}

/**
 * The energy a Supplement Dose adds, when its label states it.
 *
 * @param dose - A Meal Log's Supplement Dose.
 * @returns The kcal taken, or null when the label gives no energy.
 */
export function doseEnergy(dose: SupplementDose): number | null {
  return (
    dose.amounts.find((amount) => amount.key === "energy_kcal")?.value ?? null
  )
}

/**
 * The supplement share of a total as the Day view notes it beneath the figure, such as "2 g from
 * supplements"; nothing when no Supplement Dose adds to it.
 *
 * @param total - A day or nutrient total.
 * @returns The note, or null.
 */
export function supplementShare(total: NutrientTotal): string | null {
  if (total.supplements === null || total.supplements === 0) return null

  const unit = total.unit === null ? "" : ` ${total.unit}`

  return `${formatAmount(total.supplements)}${unit} from supplements`
}

/**
 * The calendar day `days` after `day`.
 *
 * @param day - A `YYYY-MM-DD` day.
 * @param days - Days to move; negative moves back.
 * @returns The `YYYY-MM-DD` day.
 */
export function shiftDay(day: string, days: number): string {
  const date = new Date(`${day}T00:00:00Z`)
  date.setUTCDate(date.getUTCDate() + days)

  return date.toISOString().slice(0, 10)
}

const longDay = new Intl.DateTimeFormat("en-GB", {
  timeZone: "UTC",
  weekday: "long",
  day: "numeric",
  month: "long",
})

const longDayWithYear = new Intl.DateTimeFormat("en-GB", {
  timeZone: "UTC",
  weekday: "long",
  day: "numeric",
  month: "long",
  year: "numeric",
})

/**
 * A day's name: "Today", "Yesterday", or its weekday and date, with the year when not this year.
 *
 * @param day - The `YYYY-MM-DD` day shown.
 * @param today - Today's `YYYY-MM-DD` Copenhagen day.
 * @returns The day as the page names it.
 */
export function dayName(day: string, today: string): string {
  if (day === today) return "Today"

  if (day === shiftDay(today, -1)) return "Yesterday"

  const format =
    day.slice(0, 4) === today.slice(0, 4) ? longDay : longDayWithYear

  return format.format(new Date(`${day}T12:00:00Z`))
}

const clock = new Intl.DateTimeFormat("en-GB", {
  timeZone: "Europe/Copenhagen",
  hour: "2-digit",
  minute: "2-digit",
})

/**
 * The Copenhagen clock time of a Meal Log, such as "08:15".
 *
 * @param eatenAt - Axum's RFC 3339 time with its offset.
 * @returns The local time.
 */
export function mealTime(eatenAt: string): string {
  return clock.format(new Date(eatenAt))
}

/** Energy per gram the Day view uses to split energy across macronutrients. */
const kcalPerGram = [
  ["protein", 4],
  ["available_carbohydrate", 4],
  ["fat", 9],
  ["alcohol", 7],
] as const

/** A macronutrient's share of the energy split. */
export type EnergyShare = {
  readonly key: (typeof kcalPerGram)[number][0]
  readonly kcal: number
  readonly share: number
}

/**
 * The energy from each macronutrient, using 4/4/9/7 kcal per gram; unknown totals add nothing.
 *
 * @param totals - The day's key totals.
 * @returns Each macronutrient's kcal and share of their sum, or an empty list when none is known.
 */
export function energySplit(totals: NutritionDay["totals"]): EnergyShare[] {
  const parts = kcalPerGram.map(([key, perGram]) => ({
    key,
    kcal: (totals[key].value ?? 0) * perGram,
  }))

  const sum = parts.reduce((all, part) => all + part.kcal, 0)

  if (sum <= 0) return []

  return parts.map((part) => ({ ...part, share: part.kcal / sum }))
}

/** One group of the "all nutrients" view, in the database's order. */
export type NutrientGroup = {
  readonly name: string
  readonly nutrients: readonly Nutrient[]
}

/**
 * The day's nutrients grouped as the Food database groups them, keeping database order.
 *
 * @param nutrients - Every parameter's total for the day.
 * @returns Groups in order of first appearance; values the current database no longer names go last.
 */
export function nutrientGroups(
  nutrients: readonly Nutrient[]
): NutrientGroup[] {
  const groups = new Map<string, Nutrient[]>()

  for (const item of nutrients) {
    const name = item.group?.name_en ?? "Other"
    const members = groups.get(name) ?? []
    members.push(item)
    groups.set(name, members)
  }

  return [...groups].map(([name, members]) => ({ name, nutrients: members }))
}

const twoDecimals = new Intl.NumberFormat("en-GB", { maximumFractionDigits: 2 })

const oneDecimalAmount = new Intl.NumberFormat("en-GB", {
  maximumFractionDigits: 1,
})

const wholeAmount = new Intl.NumberFormat("en-GB", { maximumFractionDigits: 0 })

/**
 * A nutrient amount with precision that suits its size: "1,234", "12.5" or "0.04".
 *
 * @param value - The amount in its unit.
 * @returns The formatted amount.
 */
export function formatAmount(value: number): string {
  const magnitude = Math.abs(value)

  if (magnitude >= 100) return wholeAmount.format(value)

  if (magnitude >= 10) return oneDecimalAmount.format(value)

  return twoDecimals.format(value)
}

/** One list of the micronutrient panel. */
export type MicronutrientGroup = {
  readonly name: string
  readonly nutrients: readonly Micronutrient[]
}

const micronutrientGroupNames: Record<Micronutrient["group"], string> = {
  vitamins: "Vitamins",
  minerals: "Minerals",
  fatty_acids: "Omega-3",
}

/**
 * The panel's nutrients in its lists, keeping Axum's order: vitamins, minerals, then EPA+DHA.
 *
 * @param nutrients - The panel's nutrients.
 * @returns The non-empty lists in order.
 */
export function micronutrientGroups(
  nutrients: readonly Micronutrient[]
): MicronutrientGroup[] {
  return (["vitamins", "minerals", "fatty_acids"] as const).flatMap((group) => {
    const members = nutrients.filter((nutrient) => nutrient.group === group)

    return members.length === 0
      ? []
      : [{ name: micronutrientGroupNames[group], nutrients: members }]
  })
}

/**
 * Whom the panel's references are for: "women 25–50 years", "men over 70".
 *
 * @param sex - The Profile's sex.
 * @param band - Axum's NNR 2023 age band, such as "25-50" or "71+".
 * @returns The group in words.
 */
export function referenceGroup(sex: "female" | "male", band: string): string {
  const people = sex === "female" ? "women" : "men"

  if (band === "71+") return `${people} over 70`

  return `${people} ${band.replace("-", "–")} years`
}

/**
 * How far the day reached the Recommended Intake, as the panel's bar fills: full at 100 % and
 * beyond, since passing it is good.
 *
 * @param nutrient - A panel nutrient.
 * @returns The bar's fill from 0 to 1, or null when the amount is unknown.
 */
export function intakeShare(nutrient: Micronutrient): number | null {
  if (nutrient.percent_of_recommended === null) return null

  return Math.min(nutrient.percent_of_recommended / 100, 1)
}

/**
 * The percentage of the Recommended Intake as the panel shows it: "64 %", "113 %".
 *
 * @param percent - Axum's percentage.
 * @returns The whole percentage reached.
 */
export function formatPercent(percent: number): string {
  return `${wholeAmount.format(Math.floor(percent))} %`
}

const limitShareNames: Record<LimitShare, string> = {
  total: "",
  supplements: " from supplements",
  preformed_vitamin_a: " as retinol",
  supplemental_folate: " as folic acid from supplements",
  nicotinic_acid: " as nicotinic acid from supplements",
  nicotinamide: " as nicotinamide from supplements",
}

/**
 * The panel's line for a nutrient's most serious Upper Limit: "Cut down: 7-day average 150 of
 * 100 µg" or "Above the Upper Limit today: 300 of 250 mg from supplements". Nothing while every
 * limit is clear or the nutrient has none.
 *
 * @param nutrient - A panel nutrient.
 * @returns The line, or null.
 */
export function upperLimitNote(nutrient: Micronutrient): string | null {
  const worst =
    nutrient.upper_limits.find((limit) => limit.status === "cut_down") ??
    nutrient.upper_limits.find((limit) => limit.status === "caution")

  if (worst === undefined) return null

  const share = limitShareNames[worst.applies_to]
  const limit = `${formatAmount(worst.limit)} ${nutrient.unit}${share}`

  if (worst.status === "cut_down" && worst.seven_day_average !== null)
    return `Cut down: 7-day average ${formatAmount(worst.seven_day_average)} of ${limit}`

  if (worst.amount === null) return null

  return `Above the Upper Limit today: ${formatAmount(worst.amount)} of ${limit}`
}
