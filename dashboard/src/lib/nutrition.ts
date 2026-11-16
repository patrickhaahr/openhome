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

const limitStatus = z.enum(["none", "caution", "cut_down"])

/**
 * Where a nutrient stands against an EFSA Upper Limit: clear, above it that day, or above it on
 * the trailing 7-day average.
 */
export type LimitStatus = z.infer<typeof limitStatus>

const limitShare = z.enum([
  "total",
  "supplements",
  "preformed_vitamin_a",
  "supplemental_folate",
  "nicotinic_acid",
  "nicotinamide",
])

/** The share of intake an Upper Limit applies to. */
export type LimitShare = z.infer<typeof limitShare>

const upperLimit = z.object({
  applies_to: limitShare,
  limit: z.number(),
  amount: z.number().nullable(),
  seven_day_average: z.number().nullable(),
  averaged_days: z.number().int().nonnegative(),
  status: limitStatus,
  source: z.string(),
})

/** One Upper Limit with the day's amount of its share and the share's trailing average. */
export type UpperLimit = z.infer<typeof upperLimit>

const micronutrient = z.object({
  key: z.string(),
  name: z.string(),
  group: z.enum(["vitamins", "minerals", "fatty_acids"]),
  unit: z.string(),
  amount: z.number().nullable(),
  food: z.number().nullable(),
  supplements: z.number().nullable(),
  unknown_portions: z.number().int().nonnegative(),
  recommended_intake: z.number().positive(),
  adequate_intake: z.boolean(),
  percent_of_recommended: z.number().nullable(),
  source: z.string(),
  upper_limits: z.array(upperLimit),
  upper_limit_status: limitStatus.nullable(),
})

/** One vitamin, mineral or EPA+DHA against its Recommended Intake and Upper Limits. */
export type Micronutrient = z.infer<typeof micronutrient>

/** The day's micronutrient panel as Axum evaluates it, or why there is none. */
const micronutrients = z.discriminatedUnion("state", [
  z.object({
    state: z.literal("available"),
    sex: z.enum(["female", "male"]),
    age: z.number().int().nonnegative(),
    age_band: z.string(),
    reference_energy_mj: z.number().positive(),
    trailing_window: z.object({
      from: z.iso.date(),
      to: z.iso.date(),
      logged_days: z.number().int().nonnegative(),
    }),
    nutrients: z.array(micronutrient),
    sources: z.array(z.object({ name: z.string(), citation: z.string() })),
  }),
  z.object({
    state: z.literal("unavailable"),
    missing: z.array(z.enum(["sex", "birth_date"])),
    reason: z.string(),
  }),
])

/** A day's micronutrient panel; the dashboard only formats it. */
export type Micronutrients = z.infer<typeof micronutrients>

/** Axum's Nutrition Day: the day's Meal Logs and totals; the dashboard only formats them. */
export const nutritionDay = z.object({
  day: z.iso.date(),
  time_zone: z.literal("Europe/Copenhagen"),
  day_start: z.iso.datetime({ offset: true }),
  day_end: z.iso.datetime({ offset: true }),
  totals: keyNutrients(total),
  targets,
  micronutrients,
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

export * from "@/lib/nutrition-display"
