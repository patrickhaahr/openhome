import { z } from "zod"

const source = z.enum(["food", "label", "estimate"])

/** A Portion's amount of one nutrient; a null value is unknown, never zero. */
const amount = z.object({
  value: z.number().nullable(),
  unit: z.string().nullable(),
  source: source.nullable(),
})

/**
 * A sum over Portions and Supplement Doses: the known values added, the Portions without one
 * counted, and the food and supplement shares of the sum.
 */
const total = z.object({
  value: z.number().nullable(),
  unit: z.string().nullable(),
  unknown_portions: z.number().int().nonnegative(),
  food: z.number().nullable(),
  supplements: z.number().nullable(),
})

/** One nutrient's total over Portions and Supplement Doses, as Axum adds them. */
export type NutrientTotal = z.infer<typeof total>

function keyNutrients<T extends z.ZodType>(schema: T) {
  return z.object({
    energy_kcal: schema,
    protein: schema,
    available_carbohydrate: schema,
    fat: schema,
    dietary_fibre: schema,
    alcohol: schema,
  })
}

/** The Product a Portion was logged as, named as its label named it then. */
const portionProduct = z.object({
  barcode: z.string(),
  name: z.string().nullable(),
  brand: z.string().nullable(),
})

/** A NOVA processing class: 1 unprocessed or minimally processed through 4 ultra-processed. */
const novaClass = z.union([
  z.literal(1),
  z.literal(2),
  z.literal(3),
  z.literal(4),
])

/** A NOVA Class as Axum reports it for a Food, Product or Portion. */
export type NovaClass = z.infer<typeof novaClass>

/** What each NOVA Class means, as the Day view explains a Portion's class. */
export const novaClassMeaning: Record<NovaClass, string> = {
  1: "unprocessed or minimally processed",
  2: "processed culinary ingredient",
  3: "processed",
  4: "ultra-processed",
}

const portion = z.object({
  food_id: z.number().int(),
  product: portionProduct.nullable(),
  name_da: z.string(),
  name_en: z.string(),
  grams: z.number(),
  dataset_version: z.string(),
  nova_class: novaClass.nullable(),
  nutrients: keyNutrients(amount),
})

/** One Portion of a Meal Log with its key nutrients scaled to the grams eaten. */
export type Portion = z.infer<typeof portion>

const mealPhoto = z.object({
  id: z.uuid(),
  width: z.number().int().positive(),
  height: z.number().int().positive(),
  thumbnail: z.object({
    width: z.number().int().positive(),
    height: z.number().int().positive(),
  }),
})

/** A Meal Log's stored AVIF photo and its thumbnail, with their pixel sizes. */
export type MealPhoto = z.infer<typeof mealPhoto>

/** Which stored file of a meal photo to show. */
export type MealPhotoSize = "photo" | "thumbnail"

/**
 * A meal photo relayed through the dashboard, so the API key stays on the server. A photo ID
 * always names the same image, so the address never needs a version.
 *
 * @param photo - The Meal Log's photo.
 * @param size - The full photo or its thumbnail.
 * @returns The dashboard path of the image.
 */
export function mealPhotoUrl(photo: MealPhoto, size: MealPhotoSize): string {
  return `/meal-photo/${photo.id}/${size}`
}

const doseUnit = z.enum(["tablet", "capsule", "scoop", "ml"])

/** What one dose of a Supplement is. */
export type DoseUnit = z.infer<typeof doseUnit>

const compound = z.object({
  name: z.string(),
  value: z.number(),
  unit: z.string(),
})

/** A named compound without a database parameter, such as creatine, with the amount taken. */
export type Compound = z.infer<typeof compound>

const supplementDose = z.object({
  supplement_id: z.string(),
  name: z.string(),
  dose_unit: doseUnit,
  doses: z.number().positive(),
  amounts: z.array(
    z.object({
      key: z.string(),
      form: z.string().nullable(),
      value: z.number(),
      unit: z.string(),
    })
  ),
  compounds: z.array(compound),
})

/** A Supplement Dose of a Meal Log: the Supplement as logged, with its amounts taken. */
export type SupplementDose = z.infer<typeof supplementDose>

const mealLog = z.object({
  id: z.string(),
  eaten_at: z.iso.datetime({ offset: true }),
  day: z.iso.date(),
  meal_name: z.string().nullable(),
  note: z.string().nullable(),
  photo: mealPhoto.nullable(),
  portions: z.array(portion),
  supplement_doses: z.array(supplementDose),
  totals: keyNutrients(total),
})

/**
 * One eating occasion at its Copenhagen time, holding its own copy of its Portions and Supplement
 * Doses.
 */
export type MealLog = z.infer<typeof mealLog>

const group = z.object({ name_da: z.string(), name_en: z.string() })

const nutrient = total.extend({
  key: z.string(),
  name_da: z.string().nullable(),
  name_en: z.string().nullable(),
  group: group.nullable(),
  supplement_forms: z
    .array(z.object({ form: z.string(), value: z.number() }))
    .optional(),
})

/** One parameter's total for the day, named as the Food database names it. */
export type Nutrient = z.infer<typeof nutrient>

const goalDirection = z.enum(["lose", "maintain", "lean_bulk"])

/** A Nutrition Goal from the day it took effect; `effective_from` is null for the default. */
const datedGoal = z.object({
  direction: goalDirection,
  rate_kg_per_week: z.number().nonnegative(),
  effective_from: z.iso.date().nullable(),
})

/** A Nutrition Goal as Axum reports it with the day it took effect. */
export type DatedGoal = z.infer<typeof datedGoal>

const macroStatus = z.enum(["met", "below"])

/** A day's Calorie Target and Macro Targets beside what was eaten, or what they need. */
const targets = z.discriminatedUnion("state", [
  z.object({
    state: z.literal("available"),
    week_start: z.iso.date(),
    week_fixed: z.boolean(),
    trend_weight_kg: z.number().positive(),
    bmr_kcal: z.number(),
    estimated_expenditure_kcal: z.number(),
    expenditure_basis: z.object({
      window_start: z.iso.date(),
      window_end: z.iso.date(),
      logged_days: z.number().int().nonnegative(),
      data_estimate_kcal: z.number().nullable(),
      prior_kcal: z.number(),
      blend_weight: z.number().min(0).max(1),
    }),
    goal: datedGoal,
    calorie_target_kcal: z.number(),
    bmr_floor_applied: z.boolean(),
    eaten_kcal: z.number(),
    remaining_kcal: z.number(),
    macros: z.object({
      protein: z.object({
        target_g: z.number(),
        eaten_g: z.number(),
        status: macroStatus,
      }),
      fat: z.object({
        floor_g: z.number(),
        eaten_g: z.number(),
        status: macroStatus,
      }),
      carbs: z.object({ target_g: z.number(), eaten_g: z.number() }),
    }),
  }),
  z.object({
    state: z.literal("unavailable"),
    missing: z.array(z.enum(["height", "sex", "birth_date", "body_weight"])),
    reason: z.string(),
  }),
])

/** A day's targets as Axum computes them; the dashboard only formats them. */
export type DayTargets = z.infer<typeof targets>

/** Axum's Nutrition Day: the day's Meal Logs and totals; the dashboard only formats them. */
export const nutritionDay = z.object({
  day: z.iso.date(),
  time_zone: z.literal("Europe/Copenhagen"),
  day_start: z.iso.datetime({ offset: true }),
  day_end: z.iso.datetime({ offset: true }),
  totals: keyNutrients(total),
  targets,
  nutrients: z.array(nutrient),
  compounds: z.array(compound),
  supplement_doses: z.array(
    z.object({
      supplement_id: z.string(),
      name: z.string(),
      dose_unit: doseUnit,
      doses: z.number().positive(),
    })
  ),
  nova: z.object({
    nova_1_share: z.number().min(0).max(1).nullable(),
    nova_1_kcal: z.number().nonnegative(),
    classified_kcal: z.number().nonnegative(),
    unclassified_portions: z.number().int().nonnegative(),
  }),
  meal_logs: z.array(mealLog),
  source: z.object({
    name: z.string(),
    version: z.string(),
    credit: z.string(),
    licence: z.string(),
  }),
  label_source: z.object({
    name: z.string(),
    url: z.string(),
    credit: z.string(),
    licence: z.string(),
  }),
})

/** One Copenhagen day of nutrition as Axum serves it. */
export type NutritionDay = z.infer<typeof nutritionDay>

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

/** The goal in effect today and every change, as Axum serves them. */
export const nutritionGoal = z.object({
  goal: datedGoal,
  history: z.array(datedGoal),
})

/** The Nutrition Goal today and its history. */
export type NutritionGoal = z.infer<typeof nutritionGoal>

/** The goals the user can choose, in the order the goal control offers them. */
export const goalChoices = [
  { key: "lose-0.75", direction: "lose", rate_kg_per_week: 0.75 },
  { key: "lose-0.5", direction: "lose", rate_kg_per_week: 0.5 },
  { key: "lose-0.25", direction: "lose", rate_kg_per_week: 0.25 },
  { key: "maintain", direction: "maintain", rate_kg_per_week: 0 },
  { key: "lean_bulk-0.1", direction: "lean_bulk", rate_kg_per_week: 0.1 },
  { key: "lean_bulk-0.25", direction: "lean_bulk", rate_kg_per_week: 0.25 },
] as const

/** One goal the user can choose. */
export type GoalChoice = (typeof goalChoices)[number]

/** A goal to set from today, as the goal control sends it. */
export const setGoalInput = z.object({
  direction: goalDirection,
  rate_kg_per_week: z.number().nonnegative(),
})

/** A goal to set from today. */
export type SetGoalInput = z.infer<typeof setGoalInput>

/** A goal write is confirmed only by Axum's parsed answer. */
export type GoalMutation =
  | { readonly state: "saved"; readonly goal: NutritionGoal }
  | { readonly state: "failed"; readonly reason: NutritionFailure }

/**
 * The choice matching a goal, so the control can show the goal in effect.
 *
 * @param goal - A goal's direction and rate.
 * @returns The matching choice, or undefined for a rate the control does not offer.
 */
export function goalChoice(
  goal: Pick<DatedGoal, "direction" | "rate_kg_per_week">
): GoalChoice | undefined {
  return goalChoices.find(
    (choice) =>
      choice.direction === goal.direction &&
      choice.rate_kg_per_week === goal.rate_kg_per_week
  )
}

/**
 * A goal in words: "Lose 0.5 kg a week", "Maintain" or "Lean bulk 0.25 kg a week".
 *
 * @param goal - A goal's direction and rate.
 * @returns The goal's name.
 */
export function goalName(
  goal: Pick<DatedGoal, "direction" | "rate_kg_per_week">
): string {
  switch (goal.direction) {
    case "maintain":
      return "Maintain"
    case "lose":
      return `Lose ${goal.rate_kg_per_week} kg a week`
    case "lean_bulk":
      return `Lean bulk ${goal.rate_kg_per_week} kg a week`
  }
}

/** Axum's answer while the Food database is importing or after a failed import. */
export const foodDatabaseUnavailable = z.object({
  code: z.literal("food_database_unavailable"),
  state: z.enum(["importing", "failed"]),
})

/** Search parameters of the Food page: the day shown, today when absent. */
export const nutritionSearch = z.object({
  day: z.iso.date().optional().catch(undefined),
})

/** The Food page's search parameters. */
export type NutritionSearch = z.infer<typeof nutritionSearch>

/** Sanitized transport and API outcomes; private diagnostics never reach the display. */
export type NutritionFailure =
  | "missing-configuration"
  | "invalid-configuration"
  | "rejected-credential"
  | "unreachable"
  | "unavailable"
  | "invalid"

/** The Day view's read: a day, the Food database's own unavailability, or another failure. */
export type NutritionDayState = {
  readonly day: string
  readonly today: string
} & (
  | {
      readonly state: "available"
      readonly data: NutritionDay
      /** Today's goal for the goal control; null when it could not be read. */
      readonly goal: NutritionGoal | null
    }
  | {
      readonly state: "food-database-unavailable"
      readonly database: z.infer<typeof foodDatabaseUnavailable>["state"]
    }
  | { readonly state: "unavailable"; readonly reason: NutritionFailure }
)

/** Actionable copy without household configuration or upstream error bodies. */
export const nutritionFailureMessage: Record<NutritionFailure, string> = {
  "missing-configuration":
    "Connect OpenHome in Connection before opening Food.",
  "invalid-configuration": "Check the private OpenHome setup in Connection.",
  "rejected-credential": "OpenHome rejected the API key. Check Connection.",
  unreachable: "OpenHome did not answer. Reopen the page to try again.",
  unavailable:
    "OpenHome could not read this day. Reopen the page to try again.",
  invalid: "This day cannot be shown. Go back to today.",
}

/** Credit for the Food database wherever nutrient values are shown, before Axum names a version. */
export const foodDatabaseCredit = "© The Danish Food Composition Database"

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
