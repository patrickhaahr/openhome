// @vitest-environment happy-dom
// @vitest-environment-options {"settings":{"fetch":{"disableSameOriginPolicy":true}}}
// The private server transport calls Axum; browser CORS applies only to Start RPCs.
import { act } from "react"
import {
  createMemoryHistory,
  createRootRoute,
  createRouter,
  RouterContextProvider,
} from "@tanstack/react-router"
import { createRoot, type Root } from "react-dom/client"
import {
  afterAll,
  afterEach,
  beforeAll,
  beforeEach,
  expect,
  test,
  vi,
} from "vitest"
import { z } from "zod"
import { FoodDay } from "@/components/food-day"
import { mealTime, shiftDay, type NutritionDayState } from "@/lib/nutrition"
import { readNutritionDay } from "@/lib/nutrition.server"
import { trainingDayOn } from "@/lib/workout"
import { startAxumFixture } from "./support/axum-fixture"

const apiKey = "synthetic-dashboard-api-key"

let ready: Awaited<ReturnType<typeof startAxumFixture>>

let importing: Awaited<ReturnType<typeof startAxumFixture>>

let root: Root

let container: HTMLDivElement

const logged: string[] = []

beforeAll(async () => {
  ready = await startAxumFixture()
  importing = await startAxumFixture(["--food-database=importing"])
})

afterAll(async () => {
  await ready.stop()
  await importing.stop()
})

beforeEach(() => {
  vi.stubEnv("OPENHOME_API_URL", ready.url)
  vi.stubEnv("API_KEY", apiKey)
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true)
  container = document.createElement("div")
  document.body.append(container)
  root = createRoot(container)
})

afterEach(async () => {
  await act(async () => root.unmount())
  container.remove()

  for (const id of logged.splice(0))
    await fetch(`${ready.url}/api/nutrition/meal-logs/${id}`, {
      method: "DELETE",
      headers: { Authorization: `Bearer ${apiKey}` },
    })
  vi.unstubAllEnvs()
  vi.unstubAllGlobals()
})

/** Logs a synthetic meal through the API, as the agent does, and returns its day. */
async function logMeal(meal: {
  eatenAt: Date
  note?: string
  portions: Array<{ food_id: number; grams: number }>
}): Promise<{ day: string; eaten_at: string }> {
  const response = await fetch(`${ready.url}/api/nutrition/meal-logs`, {
    method: "POST",
    headers: {
      Authorization: `Bearer ${apiKey}`,
      "Content-Type": "application/json",
    },
    body: JSON.stringify({
      eaten_at: meal.eatenAt.toISOString(),
      note: meal.note,
      portions: meal.portions,
      request_id: crypto.randomUUID(),
    }),
  })

  expect(response.status).toBe(201)

  const body = z
    .object({
      meal_log: z.object({
        id: z.string(),
        day: z.string(),
        eaten_at: z.string(),
      }),
    })
    .parse(await response.json())

  logged.push(body.meal_log.id)

  return body.meal_log
}

async function render(data: NutritionDayState): Promise<void> {
  const router = createRouter({
    routeTree: createRootRoute(),
    history: createMemoryHistory(),
    isServer: false,
  })

  await act(async () =>
    root.render(
      <RouterContextProvider router={router}>
        <FoodDay data={data} />
      </RouterContextProvider>
    )
  )
}

function normalize(value: string | null | undefined): string {
  return value?.replace(/\s+/g, " ").trim() ?? ""
}

function text(selector: string): string[] {
  return [...container.querySelectorAll(selector)].map((element) =>
    normalize(element.textContent)
  )
}

/** Each match's child elements' text, joined by spaces: a row as it reads across. */
function rows(selector: string): string[] {
  return [...container.querySelectorAll(selector)].map((element) =>
    [...element.children].map((child) => normalize(child.textContent)).join(" ")
  )
}

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
    expect(text(".food-day-name")).toEqual(["Today"])
    expect(
      container.querySelector(".food-day-key[data-disabled]")
    ).not.toBeNull()
  }

  expect(
    container
      .querySelector('a[aria-label^="Previous day"]')
      ?.getAttribute("href")
  ).toBe(`/food?day=${shiftDay(earlier.day, -1)}`)

  const meals = [...container.querySelectorAll(".food-meal")]
  expect(meals.map((meal) => meal.querySelector("time")?.textContent)).toEqual(
    earlier.day === later.day
      ? [mealTime(earlier.eaten_at), mealTime(later.eaten_at)]
      : [mealTime(earlier.eaten_at)]
  )
  expect(meals[0]?.querySelector(".food-note")?.textContent).toBe(
    "after the run"
  )
  expect(rows(".food-meals > li:first-child .food-portions li")).toEqual([
    "Skyr, 0.2 % fat 150 g 93 kcal",
    "Oat flakes 60 g 222 kcal",
  ])
  expect(text(".food-meal header .food-amount")[0]).toBe("315 kcal")

  if (earlier.day === later.day) {
    // 93 + 222 + 108 kcal. The banana has no alcohol value, so that total is a lower bound.
    expect(text(".food-energy .food-amount")).toEqual(["423"])
    expect(rows(".food-macros > div")).toEqual([
      "Protein 25.6 g",
      "Carbs 64.8 g",
      "Fat 4.86 g",
      "Fibre 9.12 g",
      "Alcohol ≥At least 0 g",
    ])
  }

  expect(container.querySelector(".food-split")?.getAttribute("role")).toBe(
    "img"
  )

  expect(container.querySelector("#food-nutrient-list")).toBeNull()
  await act(async () => {
    const button = [...container.querySelectorAll("button")].find(
      (element) => element.textContent === "All nutrients"
    )

    button?.click()
  })
  expect(text(".food-nutrient-groups h3")).toEqual([
    "Macro nutrients etc.",
    "Vitamins",
    "Minerals and inorganic substances",
  ])
  // Only the oats carry iron, so the day's iron is a lower bound.
  expect(rows(".food-nutrient-groups dl div")).toContain(
    "Iron ≥At least 2.4 mg"
  )
  expect(text(".food-credit")).toEqual([
    "Nutrient values © The Danish Food Composition Database, version 6.1 (CC BY 4.0)",
  ])
})

test("a backfilled meal shows on its own earlier day, which can step forward again", async () => {
  const yesterday = await logMeal({
    eatenAt: new Date(Date.now() - 26 * 60 * 60 * 1000),
    portions: [{ food_id: 907, grams: 200 }],
  })

  const today = trainingDayOn(new Date())

  await render(await readNutritionDay({ day: yesterday.day }))

  expect(rows(".food-portions li")).toEqual([
    "Chicken, breast, flesh, raw 200 g 220 kcal",
  ])
  expect(text(".food-day-name")[0]).not.toBe("Today")
  expect(
    container.querySelector('a[aria-label^="Next day"]')?.getAttribute("href")
  ).toBe(
    shiftDay(yesterday.day, 1) === today
      ? "/food"
      : `/food?day=${shiftDay(yesterday.day, 1)}`
  )
  expect(text(".food-today")).toEqual(["Today"])
})

test("an empty day says nothing was logged and leaves unknown totals as dashes", async () => {
  await render(await readNutritionDay({ day: "2026-01-15" }))

  expect(text(".food-empty h3")).toEqual(["Nothing logged"])
  expect(text(".food-energy [data-gap]")).toEqual(["Unknown"])
  expect(container.querySelector(".food-split")).toBeNull()
})

test("the Day view stays whole while the Food database is importing or OpenHome is unreachable", async () => {
  vi.stubEnv("OPENHOME_API_URL", importing.url)
  const data = await readNutritionDay({ day: "2026-01-15" })
  expect(data).toEqual({
    day: "2026-01-15",
    today: trainingDayOn(new Date()),
    state: "food-database-unavailable",
    database: "importing",
  })
  await render(data)

  expect(text(".food-unavailable h2")).toEqual(["Food database importing"])
  expect(text(".food-day-name")).toHaveLength(1)
  expect(text(".food-credit")[0]).toContain(
    "© The Danish Food Composition Database"
  )

  vi.stubEnv("OPENHOME_API_URL", "http://127.0.0.1:1")
  await render(await readNutritionDay({ day: "2026-01-15" }))
  expect(text(".food-unavailable h2")).toEqual(["Food unavailable"])
})
