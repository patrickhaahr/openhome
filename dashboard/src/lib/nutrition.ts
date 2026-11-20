import { z } from "zod"

const source = z.enum(["food", "label", "estimate"])

const amount = z.object({
  value: z.number().nullable(),
  unit: z.string().nullable(),
  source: source.nullable(),
})

const total = z.object({
  value: z.number().nullable(),
  unit: z.string().nullable(),
  unknown_portions: z.number().int().nonnegative(),
  food: z.number().nullable(),
  supplements: z.number().nullable(),
})

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

const portionProduct = z.object({
  barcode: z.string(),
  name: z.string().nullable(),
  brand: z.string().nullable(),
})

const novaClass = z.union([
  z.literal(1),
  z.literal(2),
  z.literal(3),
  z.literal(4),
])

export type NovaClass = z.infer<typeof novaClass>

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

export type MealPhoto = z.infer<typeof mealPhoto>

export type MealPhotoSize = "photo" | "thumbnail"

export function mealPhotoUrl(photo: MealPhoto, size: MealPhotoSize): string {
  return `/meal-photo/${photo.id}/${size}`
}

const doseUnit = z.enum(["tablet", "capsule", "scoop", "ml"])

export type DoseUnit = z.infer<typeof doseUnit>

const compound = z.object({
  name: z.string(),
  value: z.number(),
  unit: z.string(),
})

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

export type Nutrient = z.infer<typeof nutrient>

const goalDirection = z.enum(["lose", "maintain", "lean_bulk"])

const datedGoal = z.object({
  direction: goalDirection,
  rate_kg_per_week: z.number().nonnegative(),
  effective_from: z.iso.date().nullable(),
})

export type DatedGoal = z.infer<typeof datedGoal>

const macroStatus = z.enum(["met", "below"])

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

export type DayTargets = z.infer<typeof targets>

const limitStatus = z.enum(["none", "caution", "cut_down"])

export type LimitStatus = z.infer<typeof limitStatus>

const limitShare = z.enum([
  "total",
  "supplements",
  "preformed_vitamin_a",
  "supplemental_folate",
  "nicotinic_acid",
  "nicotinamide",
])

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

export type Micronutrient = z.infer<typeof micronutrient>

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

export type Micronutrients = z.infer<typeof micronutrients>

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

export type NutritionDay = z.infer<typeof nutritionDay>

export const nutritionGoal = z.object({
  goal: datedGoal,
  history: z.array(datedGoal),
})

export type NutritionGoal = z.infer<typeof nutritionGoal>

export const goalChoices = [
  { key: "lose-0.75", direction: "lose", rate_kg_per_week: 0.75 },
  { key: "lose-0.5", direction: "lose", rate_kg_per_week: 0.5 },
  { key: "lose-0.25", direction: "lose", rate_kg_per_week: 0.25 },
  { key: "maintain", direction: "maintain", rate_kg_per_week: 0 },
  { key: "lean_bulk-0.1", direction: "lean_bulk", rate_kg_per_week: 0.1 },
  { key: "lean_bulk-0.25", direction: "lean_bulk", rate_kg_per_week: 0.25 },
] as const

export type GoalChoice = (typeof goalChoices)[number]

export const setGoalInput = z.object({
  direction: goalDirection,
  rate_kg_per_week: z.number().nonnegative(),
})

export type SetGoalInput = z.infer<typeof setGoalInput>

export type GoalMutation =
  | { readonly state: "saved"; readonly goal: NutritionGoal }
  | { readonly state: "failed"; readonly reason: NutritionFailure }

export function goalChoice(
  goal: Pick<DatedGoal, "direction" | "rate_kg_per_week">
): GoalChoice | undefined {
  return goalChoices.find(
    (choice) =>
      choice.direction === goal.direction &&
      choice.rate_kg_per_week === goal.rate_kg_per_week
  )
}

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

export const foodDatabaseUnavailable = z.object({
  code: z.literal("food_database_unavailable"),
  state: z.enum(["importing", "failed"]),
})

export const nutritionSearch = z.object({
  day: z.iso.date().optional().catch(undefined),
})

export type NutritionSearch = z.infer<typeof nutritionSearch>

export type NutritionFailure =
  | "missing-configuration"
  | "invalid-configuration"
  | "rejected-credential"
  | "unreachable"
  | "unavailable"
  | "invalid"

export type NutritionDayState = {
  readonly day: string
  readonly today: string
} & (
  | {
      readonly state: "available"
      readonly data: NutritionDay
      readonly goal: NutritionGoal | null
    }
  | {
      readonly state: "food-database-unavailable"
      readonly database: z.infer<typeof foodDatabaseUnavailable>["state"]
    }
  | { readonly state: "unavailable"; readonly reason: NutritionFailure }
)

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

export const foodDatabaseCredit = "© The Danish Food Composition Database"

export * from "@/lib/nutrition-display"
