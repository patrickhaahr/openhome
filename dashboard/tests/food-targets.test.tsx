// @vitest-environment happy-dom
// @vitest-environment-options {"settings":{"fetch":{"disableSameOriginPolicy":true}}}
import { act } from "react"
import { expect, test, vi } from "vitest"
import { z } from "zod"
import { shiftDay } from "@/lib/nutrition"
import { readNutritionDay } from "@/lib/nutrition.server"
import { trainingDayOn } from "@/lib/workout"
import { startAxumFixture } from "./support/axum-fixture"
import {
  apiKey,
  container,
  goalWork,
  logMeal,
  normalize,
  refreshWork,
  render,
  rows,
  text,
  write,
} from "./support/food-dom"

test("the goal control starts from the goal once it can be read", async () => {
  const data = await readNutritionDay({ day: trainingDayOn(new Date()) })

  if (data.state !== "available") throw new Error(`The day is ${data.state}`)
  await render({ ...data, goal: null })
  expect(text("[data-slot='food-goal']")).toEqual([
    "The Nutrition Goal could not be read. Reopen the page to change it.",
  ])

  await render(data)

  const goal = container.querySelector("[data-slot='food-goal'] select")

  if (!(goal instanceof HTMLSelectElement)) throw new Error("Missing goal")

  if (data.goal === null) throw new Error("Missing authoritative goal")

  expect(goal.value).toBe(
    data.goal.goal.direction === "maintain"
      ? "maintain"
      : `${data.goal.goal.direction}-${data.goal.goal.rate_kg_per_week}`
  )

  const save = [...container.querySelectorAll("button")].find(
    (element) => element.textContent === "Save goal"
  )

  expect(save?.hasAttribute("disabled")).toBe(true)
  await render({
    ...data,
    goal: {
      ...data.goal,
      goal: {
        ...data.goal.goal,
        direction: "lose",
        rate_kg_per_week: 0.5,
      },
    },
  })
  expect(goal.value).toBe("lose-0.5")
  await act(async () => {
    goal.value = "lose-0.75"
    goal.dispatchEvent(new Event("change", { bubbles: true }))
  })
  expect(save?.hasAttribute("disabled")).toBe(false)
})

test("the day shows its Calorie Target, eaten and remaining with macro progress, and saves the goal from today", async () => {
  const targets = await startAxumFixture()

  try {
    vi.stubEnv("OPENHOME_API_URL", targets.url)
    const today = trainingDayOn(new Date())
    await render(await readNutritionDay({ day: today }))
    expect(text("[data-slot='food-targets'] h2")).toEqual([
      "No Calorie Target yet",
    ])
    expect(text("[data-slot='food-targets'] p")[0]).toBe(
      `Targets need the Profile's height, sex (male or female) and birth date, and a Body Weight on or before ${today}.`
    )

    const birthday = shiftDay(today, -120)

    const birthDate =
      `${Number(birthday.slice(0, 4)) - 30}${birthday.slice(4)}`.replace(
        "-02-29",
        "-02-28"
      )

    expect(
      await write(targets.url, "PATCH", "/api/profile", {
        height_cm: 180,
        sex: "male",
        birth_date: birthDate,
      })
    ).toBe(200)
    expect(
      await write(targets.url, "POST", "/api/body_weight", {
        date: shiftDay(today, -20),
        weight_kg: 80,
      })
    ).toBe(201)

    const meal = await logMeal({
      api: targets.url,
      eatenAt: new Date(Date.now() - 1000),
      portions: [{ food_id: 907, grams: 1000 }],
    })

    await render(await readNutritionDay({ day: meal.day }))

    expect(rows("[data-slot='food-target-strip'] > div")).toEqual([
      "Target 2,120 kcal",
      "Eaten 1,100 kcal",
      "Remaining 1,020 kcal",
    ])
    expect(text("[data-slot='food-target-basis']")).toEqual([
      "Lose 0.5 kg a week · Estimated Expenditure 2,670 kcal · Trend Weight 80 kg",
    ])
    expect(text("[data-slot='food-macro-target-name']")).toEqual([
      "Protein",
      "Fat",
      "Carbs",
    ])
    expect(text("[data-slot='food-macro-target-amount']")).toEqual([
      "230 / 176 g",
      "15 / 56 g",
      "0 / 228 g",
    ])
    expect(text("[data-slot='food-macro-target-status']")).toEqual([
      "met",
      "below floor",
      "",
    ])

    const settled = refreshWork
    const goal = container.querySelector("select")

    if (!(goal instanceof HTMLSelectElement)) throw new Error("Missing goal")
    expect(goal.value).toBe("lose-0.5")
    await act(async () => {
      Object.getOwnPropertyDescriptor(
        HTMLSelectElement.prototype,
        "value"
      )?.set?.call(goal, "maintain")
      goal.dispatchEvent(new Event("change", { bubbles: true }))
    })

    const save = [...container.querySelectorAll("button")].find(
      (element) => element.textContent === "Save goal"
    )

    expect(save?.hasAttribute("disabled")).toBe(false)
    await act(async () => {
      save?.click()
      await goalWork
    })
    await act(async () => {
      await vi.waitFor(() => expect(refreshWork).not.toBe(settled))
      await refreshWork
    })
    expect(text("[data-slot='food-goal-feedback']")).toEqual([
      "Goal saved: Maintain from today.",
    ])

    expect(rows("[data-slot='food-target-strip'] > div")).toEqual([
      "Target 2,670 kcal",
      "Eaten 1,100 kcal",
      "Remaining 1,570 kcal",
    ])
    expect(text("[data-slot='food-target-basis']")[0]).toMatch(/^Maintain · /)

    const saved = await fetch(`${targets.url}/api/nutrition/goal`, {
      headers: { Authorization: `Bearer ${apiKey}` },
    })

    expect(await saved.json()).toEqual({
      goal: {
        direction: "maintain",
        rate_kg_per_week: 0,
        effective_from: today,
      },
      history: [
        { direction: "maintain", rate_kg_per_week: 0, effective_from: today },
      ],
    })
  } finally {
    await targets.stop()
  }
})

type Json =
  | string
  | number
  | boolean
  | null
  | readonly Json[]
  | { readonly [key: string]: Json }

type AgentWrite = {
  readonly request_id: string
  readonly [field: string]: Json
}

async function post<T>(
  api: string,
  path: string,
  body: AgentWrite,
  schema: z.ZodType<T>
): Promise<T> {
  const response = await fetch(`${api}${path}`, {
    method: "POST",
    headers: {
      Authorization: `Bearer ${apiKey}`,
      "Content-Type": "application/json",
    },
    body: JSON.stringify(body),
  })

  expect(response.status).toBe(201)

  return schema.parse(await response.json())
}

function micro(key: string, part: string): string {
  return normalize(
    container.querySelector(
      `[data-slot="food-micro-list"] li[data-key="${key}"] ${part}`
    )?.textContent
  )
}

test("the day shows vitamins and minerals against their Recommended Intakes, adequate intakes, the supplement split, a caution and a red cut down", async () => {
  const micros = await startAxumFixture()

  try {
    vi.stubEnv("OPENHOME_API_URL", micros.url)
    const today = trainingDayOn(new Date())
    await render(await readNutritionDay({ day: today }))
    expect(text("[data-slot='food-micros'] h2")).toEqual([
      "No vitamin and mineral panel yet",
    ])
    expect(text("[data-slot='food-micros'] p")).toEqual([
      "The micronutrient panel needs the Profile's sex (male or female) and birth date.",
      "Sex and birth date are set on the Workout page's Body view.",
    ])

    const birthday = shiftDay(today, -120)

    const birthDate =
      `${Number(birthday.slice(0, 4)) - 30}${birthday.slice(4)}`.replace(
        "-02-29",
        "-02-28"
      )

    expect(
      await write(micros.url, "PATCH", "/api/profile", {
        sex: "female",
        birth_date: birthDate,
      })
    ).toBe(200)

    const supplementId = z.object({ supplement: z.object({ id: z.string() }) })

    const vitaminD = (
      await post(
        micros.url,
        "/api/nutrition/supplements",
        {
          name: "Vitamin D 150",
          dose_unit: "capsule",
          amounts: [{ key: "vitamin_d", value: 150, unit: "µg" }],
          request_id: crypto.randomUUID(),
        },
        supplementId
      )
    ).supplement.id

    const magnesium = (
      await post(
        micros.url,
        "/api/nutrition/supplements",
        {
          name: "Magnesium",
          dose_unit: "tablet",
          amounts: [{ key: "magnesium", value: 300, unit: "mg" }],
          request_id: crypto.randomUUID(),
        },
        supplementId
      )
    ).supplement.id

    const mealLog = z.object({ meal_log: z.object({ day: z.string() }) })

    for (const daysAgo of [2, 1]) {
      await post(
        micros.url,
        "/api/nutrition/meal-logs",
        {
          eaten_at: new Date(Date.now() - daysAgo * 86_400_000).toISOString(),
          supplement_doses: [{ supplement_id: vitaminD, doses: 1 }],
          request_id: crypto.randomUUID(),
        },
        mealLog
      )
    }

    const meal = (
      await post(
        micros.url,
        "/api/nutrition/meal-logs",
        {
          eaten_at: new Date(Date.now() - 1000).toISOString(),
          portions: [{ food_id: 3, grams: 200 }],
          supplement_doses: [
            { supplement_id: vitaminD, doses: 1 },
            { supplement_id: magnesium, doses: 1 },
          ],
          request_id: crypto.randomUUID(),
        },
        mealLog
      )
    ).meal_log

    await render(await readNutritionDay({ day: meal.day }))

    expect(text("[data-slot='food-micros'] > h2")).toEqual([
      "Vitamins and minerals",
    ])
    expect(
      text("[data-slot='food-micros'] > [data-slot='food-micros-basis']")[0]
    ).toBe(
      "Recommended Intakes for women 25–50 years · Upper Limits on 3 logged days of the last 7"
    )
    expect(text("[data-slot='food-micro-group'] h3")).toEqual([
      "Vitamins",
      "Minerals",
      "Omega-3",
    ])

    expect(micro("vitamin_c", "[data-slot='food-micro-name']")).toBe(
      "Vitamin C"
    )
    expect(micro("vitamin_c", "[data-slot='food-micro-percent']")).toBe("18 %")
    expect(micro("vitamin_c", "[data-slot='food-micro-amount']")).toBe(
      "18 / 95 mg"
    )
    expect(
      container
        .querySelector('[data-slot="food-micro-list"] li[data-key="vitamin_c"]')
        ?.hasAttribute("data-limit")
    ).toBe(false)

    expect(micro("vitamin_d", "[data-slot='food-micro-percent']")).toBe(
      "1,500 %"
    )
    expect(micro("vitamin_d", "[data-slot='food-micro-amount']")).toBe(
      "150 / 10 µg"
    )
    expect(micro("vitamin_d", "[data-slot='food-micro-split']")).toBe(
      "Food unknown · supplements 150 µg"
    )
    expect(micro("vitamin_d", "[data-slot='food-micro-limit']")).toBe(
      "Cut down: 7-day average 150 of 100 µg"
    )

    expect(micro("magnesium", "[data-slot='food-micro-name']")).toBe(
      "Magnesium, adequate intake AI"
    )
    expect(micro("magnesium", "[data-slot='food-micro-percent']")).toBe("100 %")
    expect(micro("magnesium", "[data-slot='food-micro-limit']")).toBe(
      "Above the Upper Limit today: 300 of 250 mg from supplements"
    )

    expect(
      [
        ...container.querySelectorAll(
          "[data-slot='food-micro-list'] li[data-limit]"
        ),
      ].map((row) => [
        row.getAttribute("data-key"),
        row.getAttribute("data-limit"),
      ])
    ).toEqual([
      ["vitamin_d", "cut_down"],
      ["magnesium", "caution"],
    ])

    expect(micro("vitamin_k", "[data-slot='food-micro-percent']")).toBe(
      "Unknown"
    )
    expect(micro("epa_dha", "[data-slot='food-micro-name']")).toBe(
      "EPA+DHA, adequate intake AI"
    )
  } finally {
    await micros.stop()
  }
})
