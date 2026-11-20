// @vitest-environment happy-dom
// @vitest-environment-options {"settings":{"fetch":{"disableSameOriginPolicy":true}}}
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
import { FoodTrends } from "@/components/food-trends"
import { shiftDay } from "@/lib/nutrition"
import type { NutritionSummaryState } from "@/lib/nutrition-trends"
import { readNutritionSummary } from "@/lib/nutrition.server"
import { trainingDayOn } from "@/lib/workout"
import { startAxumFixture } from "./support/axum-fixture"

const apiKey = "synthetic-dashboard-api-key"

let api: Awaited<ReturnType<typeof startAxumFixture>>

let root: Root

let container: HTMLDivElement

const today = trainingDayOn(new Date())

type FixtureBody = Readonly<
  Record<
    string,
    string | number | ReadonlyArray<Readonly<Record<string, number>>>
  >
>

async function send(
  method: "POST" | "PATCH",
  path: string,
  body: FixtureBody
): Promise<void> {
  const response = await fetch(`${api.url}${path}`, {
    method,
    headers: {
      Authorization: `Bearer ${apiKey}`,
      "Content-Type": "application/json",
    },
    body: JSON.stringify(body),
  })

  expect(response.ok, await response.clone().text()).toBe(true)
}

async function eatSkyr(day: string, grams: number): Promise<void> {
  await send("POST", "/api/nutrition/meal-logs", {
    eaten_at: `${day}T11:00:00Z`,
    portions: [{ food_id: 1693, grams }],
    request_id: crypto.randomUUID(),
  })
}

beforeAll(async () => {
  api = await startAxumFixture()
  await send("PATCH", "/api/profile", {
    height_cm: 180,
    sex: "male",
    birth_date: "1990-05-01",
  })

  for (const daysAgo of [20, 13, 6, 1])
    await send("POST", "/api/body_weight", {
      date: shiftDay(today, -daysAgo),
      weight_kg: 80 - daysAgo / 10,
    })

  await eatSkyr(shiftDay(today, -3), 300)
  await eatSkyr(shiftDay(today, -1), 500)
})

afterAll(async () => {
  await api.stop()
})

beforeEach(() => {
  vi.stubEnv("OPENHOME_API_URL", api.url)
  vi.stubEnv("API_KEY", apiKey)
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true)
  container = document.createElement("div")
  document.body.append(container)
  root = createRoot(container)
})

afterEach(async () => {
  await act(async () => root.unmount())
  container.remove()
  vi.unstubAllEnvs()
  vi.unstubAllGlobals()
})

async function render(
  data: NutritionSummaryState,
  location = "/food?view=trends"
): Promise<void> {
  const router = createRouter({
    routeTree: createRootRoute(),
    history: createMemoryHistory({ initialEntries: [location] }),
    isServer: false,
  })

  await act(async () =>
    root.render(
      <RouterContextProvider router={router}>
        <FoodTrends data={data} />
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

function table(chart: string): string[][] {
  return [
    ...container.querySelectorAll(`figure[data-chart="${chart}"] table tr`),
  ].map((row) => [...row.children].map((cell) => normalize(cell.textContent)))
}

test("draws 30 days of trends with missing days as gaps and a table for each chart", async () => {
  const data = await readNutritionSummary({ range: 30, by: "day" })
  expect(data.state).toBe("available")
  await render(data)

  expect(text("[data-slot='food-credit']")).toEqual([
    "Nutrient values © The Danish Food Composition Database, version 6.1 (CC BY 4.0)",
  ])

  expect(text('nav[aria-label="Food views"] a')).toEqual(["Day", "Trends"])
  expect(text('nav[aria-label="Food views"] [aria-current="page"]')).toEqual([
    "Trends",
  ])
  expect(text('nav[aria-label="Range"] [aria-current="page"]')).toEqual([
    "30 days",
  ])
  expect(
    [...container.querySelectorAll('nav[aria-label="Range"] a')].map((link) =>
      link.getAttribute("href")
    )
  ).toEqual([
    "/food?view=trends",
    "/food?view=trends&range=90",
    "/food?view=trends&range=180",
    "/food?view=trends&range=365",
  ])
  expect(text('nav[aria-label="Group by"] [aria-current="page"]')).toEqual([
    "Days",
  ])
  expect(
    container
      .querySelector('nav[aria-label="Group by"] a:last-child')
      ?.getAttribute("href")
  ).toBe("/food?view=trends&by=month")

  expect(text("[data-slot='food-trend-coverage']")[0]).toMatch(
    /^2 of 30 days logged/
  )
  expect(text("[data-slot='trend-chart'] h2")).toEqual([
    "Calories",
    "Protein",
    "Energy by macronutrient",
    "NOVA 1 share",
    "Weight",
    "Weekly change",
  ])

  for (const svg of container.querySelectorAll("[data-slot='trend-chart'] svg"))
    expect(svg.getAttribute("aria-hidden")).toBe("true")
  expect(
    text('figure[data-chart="calories"] [data-slot="trend-summary"]')[0]
  ).toMatch(
    /^248 kcal a logged day · Calorie Target [\d,]+ kcal · Estimated Expenditure [\d,]+ kcal$/
  )

  expect(
    container.querySelectorAll(
      'figure[data-chart="calories"] rect[data-mark="eaten"]'
    )
  ).toHaveLength(2)
  const calories = table("calories")
  expect(calories[0]).toEqual([
    "Period",
    "Eaten",
    "Calorie Target",
    "Estimated Expenditure",
  ])
  expect(calories).toHaveLength(31)
  expect(calories.slice(-4).map((row) => row[1])).toEqual([
    "186 kcal",
    "Not logged",
    "310 kcal",
    "Not logged",
  ])
  expect(table("protein").at(-2)?.slice(1)).toEqual([
    "55 g",
    expect.stringMatching(/^\d+ g$/),
  ])
  expect(
    container.querySelectorAll(
      'figure[data-chart="calories"] [data-mark="target"] path'
    ).length
  ).toBeGreaterThan(0)
  expect(
    container.querySelectorAll(
      'figure[data-chart="weight"] [data-slot="trend-dot"]'
    )
  ).toHaveLength(4)
  expect(
    text('figure[data-chart="weight"] [data-slot="trend-summary"]')
  ).toEqual(["Trend Weight 78.4 kg · +0.4 kg over the range"])
})

test("groups by ISO week and names the partial edge weeks", async () => {
  const data = await readNutritionSummary({ range: 90, by: "week" })
  expect(data.state).toBe("available")
  await render(data, "/food?view=trends&range=90")

  expect(text('nav[aria-label="Food views"] [aria-current="page"]')).toEqual([
    "Trends",
  ])

  expect(text('nav[aria-label="Range"] [aria-current="page"]')).toEqual([
    "90 days",
  ])
  expect(text('nav[aria-label="Group by"] [aria-current="page"]')).toEqual([
    "Weeks",
  ])
  const rows = table("calories").slice(1)
  expect(rows.every((row) => row[0]?.startsWith("Week of") === true)).toBe(true)

  const days = rows.map((row) =>
    Number(/\((\d+) days? in range\)/.exec(row[0] ?? "")?.[1] ?? 7)
  )

  expect(days.reduce((sum, count) => sum + count, 0)).toBe(90)

  if (new Date(`${today}T12:00:00Z`).getUTCDay() !== 0)
    expect(rows.at(-1)?.[0]).toMatch(/in range\)$/)
})

test("an unreachable or rejected read shows the state note and keeps the controls", async () => {
  vi.stubEnv("API_KEY", "not-the-synthetic-key")
  const data = await readNutritionSummary({ range: 180, by: "month" })
  expect(data).toMatchObject({
    state: "unavailable",
    reason: "rejected-credential",
    range: 180,
    by: "month",
  })
  await render(data, "/food?view=trends&range=180&by=month")

  expect(text("[data-slot='food-unavailable'] h2")).toEqual([
    "Trends unavailable",
  ])
  expect(text("[data-slot='food-unavailable'] p")[0]).toMatch(/API key/)
  expect(text('nav[aria-label="Range"] [aria-current="page"]')).toEqual([
    "180 days",
  ])
})

type ProductWrite =
  | { barcode: string; food_id: number; request_id: string }
  | {
      eaten_at: string
      portions: { barcode: string; grams: number }[]
      request_id: string
    }

test("a range with a Product credits Open Food Facts beside the Food database", async () => {
  const products = await startAxumFixture()

  try {
    vi.stubEnv("OPENHOME_API_URL", products.url)

    const write = async (path: string, body: ProductWrite): Promise<void> => {
      const response = await fetch(`${products.url}${path}`, {
        method: "POST",
        headers: {
          Authorization: `Bearer ${apiKey}`,
          "Content-Type": "application/json",
        },
        body: JSON.stringify(body),
      })

      expect(response.ok, await response.clone().text()).toBe(true)
    }

    const barcode = "2001234567893"
    await write("/api/nutrition/products", {
      barcode,
      food_id: 1693,
      request_id: crypto.randomUUID(),
    })
    await write("/api/nutrition/meal-logs", {
      eaten_at: `${shiftDay(today, -1)}T11:00:00Z`,
      portions: [{ barcode, grams: 150 }],
      request_id: crypto.randomUUID(),
    })

    await render(await readNutritionSummary({ range: 30, by: "day" }))

    expect(text("[data-slot='food-credit']")).toEqual([
      "Nutrient values © The Danish Food Composition Database, version 6.1 (CC BY 4.0)",
      "Product data © Open Food Facts contributors (ODbL)",
    ])
  } finally {
    await products.stop()
  }
})
