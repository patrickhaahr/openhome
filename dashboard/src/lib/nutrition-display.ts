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

export type PortionName = {
  readonly name: string
  readonly brand: string | null
}

export function portionName(portion: Portion): PortionName {
  return {
    name: portion.product?.name ?? portion.name_en,
    brand: portion.product?.brand ?? null,
  }
}

export function hasProducts(day: NutritionDay): boolean {
  return day.meal_logs.some((meal) =>
    meal.portions.some((portion) => portion.product !== null)
  )
}

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

export function doseCount(doses: number, unit: DoseUnit): string {
  const [one, many] = doseUnitNames[unit]

  return `${formatAmount(doses)} ${doses === 1 ? one : many}`
}

export function doseEnergy(dose: SupplementDose): number | null {
  return (
    dose.amounts.find((amount) => amount.key === "energy_kcal")?.value ?? null
  )
}

export function supplementShare(total: NutrientTotal): string | null {
  if (total.supplements === null || total.supplements === 0) return null

  const unit = total.unit === null ? "" : ` ${total.unit}`

  return `${formatAmount(total.supplements)}${unit} from supplements`
}

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

export function mealTime(eatenAt: string): string {
  return clock.format(new Date(eatenAt))
}

const kcalPerGram = [
  ["protein", 4],
  ["available_carbohydrate", 4],
  ["fat", 9],
  ["alcohol", 7],
] as const

export type EnergyShare = {
  readonly key: (typeof kcalPerGram)[number][0]
  readonly kcal: number
  readonly share: number
}

export function energySplit(totals: NutritionDay["totals"]): EnergyShare[] {
  const parts = kcalPerGram.map(([key, perGram]) => ({
    key,
    kcal: (totals[key].value ?? 0) * perGram,
  }))

  const sum = parts.reduce((all, part) => all + part.kcal, 0)

  if (sum <= 0) return []

  return parts.map((part) => ({ ...part, share: part.kcal / sum }))
}

export type NutrientGroup = {
  readonly name: string
  readonly nutrients: readonly Nutrient[]
}

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

export function formatAmount(value: number): string {
  const magnitude = Math.abs(value)

  if (magnitude >= 100) return wholeAmount.format(value)

  if (magnitude >= 10) return oneDecimalAmount.format(value)

  return twoDecimals.format(value)
}

export type MicronutrientGroup = {
  readonly name: string
  readonly nutrients: readonly Micronutrient[]
}

const micronutrientGroupNames: Record<Micronutrient["group"], string> = {
  vitamins: "Vitamins",
  minerals: "Minerals",
  fatty_acids: "Omega-3",
}

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

export function referenceGroup(sex: "female" | "male", band: string): string {
  const people = sex === "female" ? "women" : "men"

  if (band === "71+") return `${people} over 70`

  return `${people} ${band.replace("-", "–")} years`
}

export function intakeShare(nutrient: Micronutrient): number | null {
  if (nutrient.percent_of_recommended === null) return null

  return Math.min(nutrient.percent_of_recommended / 100, 1)
}

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
