// @vitest-environment happy-dom
// @vitest-environment-options {"settings":{"fetch":{"disableSameOriginPolicy":true}}}
// The private server transport calls Axum; browser CORS applies only to Start RPCs.
import { act } from "react"
import { expect, test } from "vitest"
import { z } from "zod"
import { mealTime, shiftDay, type DoseUnit } from "@/lib/nutrition"
import { readNutritionDay } from "@/lib/nutrition.server"
import { trainingDayOn } from "@/lib/workout"
import {
  addedSupplements,
  apiKey,
  container,
  logged,
  logMeal,
  normalize,
  ready,
  render,
  rows,
  savedMeals,
  text,
} from "./support/food-dom"

test("opens on today with the day's Meal Logs in time order, totals, all nutrients and the credit", async () => {
  const now = Date.now()

  // Logged out of order; the day lists them by time.
  const later = await logMeal({
    eatenAt: new Date(now - 60_000),
    portions: [{ food_id: 3, grams: 120 }],
  })

  const earlier = await logMeal({
    eatenAt: new Date(now - 120_000),
    note: "after the run",
    portions: [
      { food_id: 1693, grams: 150 },
      { food_id: 60, grams: 60 },
    ],
  })

  const today = await readNutritionDay({})
  expect(today.day).toBe(trainingDayOn(new Date()))
  const data = await readNutritionDay({ day: earlier.day })
  expect(data.state).toBe("available")
  await render(data)

  if (earlier.day === today.day) {
    expect(text("[data-slot='food-day-name']")).toEqual(["Today"])
    expect(
      container.querySelector("[data-slot='food-day-key'][data-disabled]")
    ).not.toBeNull()
  }

  expect(
    container
      .querySelector('a[aria-label^="Previous day"]')
      ?.getAttribute("href")
  ).toBe(`/food?day=${shiftDay(earlier.day, -1)}`)

  const meals = [...container.querySelectorAll("[data-slot='food-meal']")]
  expect(meals.map((meal) => meal.querySelector("time")?.textContent)).toEqual(
    earlier.day === later.day
      ? [mealTime(earlier.eaten_at), mealTime(later.eaten_at)]
      : [mealTime(earlier.eaten_at)]
  )
  expect(meals[0]?.querySelector("[data-slot='food-note']")?.textContent).toBe(
    "after the run"
  )
  expect(
    rows(
      "[data-slot='food-meals'] > li:first-child [data-slot='food-portions'] li"
    )
  ).toEqual(["Skyr, 0.2 % fat 150 g 93 kcal", "Oat flakes 60 g 222 kcal"])
  expect(
    text("[data-slot='food-meal'] header [data-slot='food-amount']")[0]
  ).toBe("315 kcal")

  if (earlier.day === later.day) {
    // 93 + 222 + 108 kcal. The banana has no alcohol value, so that total is a lower bound.
    expect(text("[data-slot='food-energy'] [data-slot='food-amount']")).toEqual(
      ["423"]
    )
    expect(rows("[data-slot='food-macros'] > div")).toEqual([
      "Protein 25.6 g",
      "Carbs 64.8 g",
      "Fat 4.86 g",
      "Fibre 9.12 g",
      "Alcohol ≥At least 0 g",
    ])
  }

  expect(
    container.querySelector("[data-slot='food-split']")?.getAttribute("role")
  ).toBe("img")

  expect(container.querySelector("#food-nutrient-list")).toBeNull()
  await act(async () => {
    const button = [...container.querySelectorAll("button")].find(
      (element) => element.textContent === "All nutrients"
    )

    button?.click()
  })
  expect(text("[data-slot='food-nutrient-groups'] h3")).toEqual([
    "Macro nutrients etc.",
    "Vitamins",
    "Minerals and inorganic substances",
  ])
  // Only the oats carry iron, so the day's iron is a lower bound.
  expect(rows("[data-slot='food-nutrient-groups'] dl div")).toContain(
    "Iron ≥At least 2.4 mg"
  )
  expect(text("[data-slot='food-credit']")).toEqual([
    "Nutrient values © The Danish Food Composition Database, version 6.1 (CC BY 4.0)",
  ])
})

test("a Product Portion is named with its brand and the day credits Open Food Facts", async () => {
  // The fixture's synthetic Open Food Facts product: a restricted-circulation barcode.
  const barcode = "2001234567893"

  const linked = await fetch(`${ready.url}/api/nutrition/products`, {
    method: "POST",
    headers: {
      Authorization: `Bearer ${apiKey}`,
      "Content-Type": "application/json",
    },
    body: JSON.stringify({
      barcode,
      food_id: 1693,
      request_id: "dashboard-link-skyr",
    }),
  })

  expect([200, 201]).toContain(linked.status)

  const meal = await logMeal({
    eatenAt: new Date(Date.now() - 75_000),
    portions: [
      { barcode, grams: 150 },
      { food_id: 60, grams: 40 },
    ],
  })

  await render(await readNutritionDay({ day: meal.day }))

  const portions = [...container.querySelectorAll("[data-slot='food-meal']")]
    .find(
      (element) =>
        element.querySelector("time")?.textContent === mealTime(meal.eaten_at)
    )
    ?.querySelectorAll("[data-slot='food-portions'] li")

  // The label's 76 kcal per 100 g, not the linked Skyr's 62.
  expect(
    [...(portions ?? [])].map((row) =>
      [...row.children].map((child) => normalize(child.textContent)).join(" ")
    )
  ).toEqual([
    "Vanilla Skyr, NOVA 3, Synthetic Dairy 150 g 114 kcal",
    "Oat flakes 40 g 148 kcal",
  ])
  expect(text("[data-slot='food-portion-brand']")).toContain(
    ", Synthetic Dairy"
  )
  expect(text("[data-slot='food-credit']")).toEqual([
    "Nutrient values © The Danish Food Composition Database, version 6.1 (CC BY 4.0)",
    "Product data © Open Food Facts contributors (ODbL)",
  ])
})

test("a Meal Log copied from a saved Meal names the Meal, and a one-off meal names none", async () => {
  const headers = {
    Authorization: `Bearer ${apiKey}`,
    "Content-Type": "application/json",
  }

  const saved = await fetch(`${ready.url}/api/nutrition/meals`, {
    method: "POST",
    headers,
    body: JSON.stringify({
      name: "Protein oats",
      portions: [
        { food_id: 1693, grams: 150 },
        { food_id: 60, grams: 50 },
      ],
      request_id: crypto.randomUUID(),
    }),
  })

  expect(saved.status).toBe(201)
  savedMeals.push(
    z.object({ meal: z.object({ id: z.string() }) }).parse(await saved.json())
      .meal.id
  )

  const response = await fetch(`${ready.url}/api/nutrition/meal-logs`, {
    method: "POST",
    headers,
    body: JSON.stringify({
      eaten_at: new Date(Date.now() - 90_000).toISOString(),
      meal: "protein oats",
      overrides: [{ food_id: 60, grams: 70 }],
      request_id: crypto.randomUUID(),
    }),
  })

  expect(response.status).toBe(201)

  const fromMeal = z
    .object({
      meal_log: z.object({
        id: z.string(),
        day: z.string(),
        eaten_at: z.string(),
      }),
    })
    .parse(await response.json()).meal_log

  logged.push(fromMeal.id)

  const oneOff = await logMeal({
    eatenAt: new Date(Date.now() - 30_000),
    portions: [{ food_id: 3, grams: 120 }],
  })

  await render(await readNutritionDay({ day: fromMeal.day }))

  const plate = (eatenAt: string) =>
    [...container.querySelectorAll("[data-slot='food-meal']")].find(
      (element) =>
        element.querySelector("time")?.textContent === mealTime(eatenAt)
    )

  expect(
    normalize(
      plate(fromMeal.eaten_at)?.querySelector("[data-slot='food-meal-from']")
        ?.textContent
    )
  ).toBe("Logged from the Meal Protein oats")
  expect(
    [
      ...(plate(fromMeal.eaten_at)?.querySelectorAll(
        "[data-slot='food-portions'] li"
      ) ?? []),
    ].map((row) =>
      [...row.children].map((child) => normalize(child.textContent)).join(" ")
    )
  ).toEqual(["Skyr, 0.2 % fat 150 g 93 kcal", "Oat flakes 70 g 259 kcal"])

  if (oneOff.day === fromMeal.day)
    expect(
      plate(oneOff.eaten_at)?.querySelector("[data-slot='food-meal-from']")
    ).toBeNull()
})

/** A synthetic Supplement's label, per dose. */
type SyntheticSupplement = {
  readonly name: string
  readonly dose_unit: DoseUnit
  readonly amounts?: ReadonlyArray<{ key: string; value: number; unit: string }>
  readonly compounds?: ReadonlyArray<{
    name: string
    value: number
    unit: string
  }>
}

/** Adds a synthetic Supplement through the API, as the agent does from a label. */
async function addSupplement(body: SyntheticSupplement): Promise<string> {
  const response = await fetch(`${ready.url}/api/nutrition/supplements`, {
    method: "POST",
    headers: {
      Authorization: `Bearer ${apiKey}`,
      "Content-Type": "application/json",
    },
    body: JSON.stringify({ ...body, request_id: crypto.randomUUID() }),
  })

  expect(response.status).toBe(201)

  const id = z
    .object({ supplement: z.object({ id: z.string() }) })
    .parse(await response.json()).supplement.id

  addedSupplements.push(id)

  return id
}

test("the day shows a Supplements row, each meal's Supplement Doses and the supplement share of its totals", async () => {
  const fishOil = await addSupplement({
    name: "Fish oil",
    dose_unit: "capsule",
    amounts: [
      { key: "epa", value: 180, unit: "mg" },
      { key: "energy_kcal", value: 9, unit: "kcal" },
      { key: "fat", value: 1, unit: "g" },
    ],
  })

  const vitaminD = await addSupplement({
    name: "Vitamin D3",
    dose_unit: "capsule",
    amounts: [{ key: "vitamin_d", value: 25, unit: "µg" }],
  })

  const creatine = await addSupplement({
    name: "Creatine",
    dose_unit: "scoop",
    compounds: [{ name: "Creatine monohydrate", value: 3, unit: "g" }],
  })

  const response = await fetch(`${ready.url}/api/nutrition/meal-logs`, {
    method: "POST",
    headers: {
      Authorization: `Bearer ${apiKey}`,
      "Content-Type": "application/json",
    },
    body: JSON.stringify({
      eaten_at: new Date(Date.now() - 45_000).toISOString(),
      portions: [{ food_id: 1693, grams: 150 }],
      supplement_doses: [
        { supplement_id: fishOil, doses: 2 },
        { supplement_id: vitaminD, doses: 1 },
        { supplement_id: creatine, doses: 1 },
      ],
      request_id: crypto.randomUUID(),
    }),
  })

  expect(response.status).toBe(201)

  const meal = z
    .object({ meal_log: z.object({ id: z.string(), day: z.string() }) })
    .parse(await response.json()).meal_log

  logged.push(meal.id)
  await render(await readNutritionDay({ day: meal.day }))

  expect(
    normalize(
      container.querySelector("[data-slot='food-supplements'] h3")?.textContent
    )
  ).toBe("Supplements")
  expect(rows("[data-slot='food-supplements'] li")).toEqual([
    "Fish oil 2 capsules",
    "Vitamin D3 1 capsule",
    "Creatine 1 scoop",
    "Creatine monohydrate 3 g",
  ])
  expect(
    rows("[data-slot='food-meal'] [data-slot='food-doses'] li").map((row) =>
      row.trim()
    )
  ).toEqual([
    "Fish oil 2 capsules 18 kcal",
    "Vitamin D3 1 capsule",
    "Creatine 1 scoop",
  ])
  // The skyr's 93 kcal and 0.3 g fat, plus what the fish oil's label states.
  expect(
    text("[data-slot='food-meal'] header [data-slot='food-amount']")
  ).toContain("111 kcal")
  expect(text("[data-slot='food-energy-supplements']")).toEqual([
    "18 kcal from supplements",
  ])
  expect(rows('[data-slot="food-macros"] > div[data-macro="fat"]')).toEqual([
    "Fat 2.3 g 2 g from supplements",
  ])

  await act(async () => {
    const button = [...container.querySelectorAll("button")].find(
      (element) => element.textContent === "All nutrients"
    )

    button?.click()
  })
  expect(rows("[data-slot='food-nutrient-groups'] dl div")).toContain(
    "Vitamin D 25 µg Food 0 µg · supplements 25 µg"
  )
})
