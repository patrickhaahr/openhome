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
import { z } from "zod"
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

// Its own fixture: the Profile, Supplement and Meal Logs outlive the test.
let api: Awaited<ReturnType<typeof startAxumFixture>>

let root: Root

let container: HTMLDivElement

const today = trainingDayOn(new Date())

/** A synthetic JSON request body. */
type Json =
  | string
  | number
  | boolean
  | null
  | readonly Json[]
  | { readonly [key: string]: Json }

async function send(
  method: "POST" | "PATCH",
  path: string,
  body: Json
): Promise<string> {
  const response = await fetch(`${api.url}${path}`, {
    method,
    headers: {
      Authorization: `Bearer ${apiKey}`,
      "Content-Type": "application/json",
    },
    body: JSON.stringify(body),
  })

  const text = await response.text()
  expect(response.ok, text).toBe(true)

  return text
}

beforeAll(async () => {
  api = await startAxumFixture()
  // A man of 36: NNR 2023 25–50 years, calcium 950 mg, vitamin D 10 µg (Upper Limit 100 µg).
  await send("PATCH", "/api/profile", { sex: "male", birth_date: "1990-05-01" })

  const added = await send("POST", "/api/nutrition/supplements", {
    name: "Vitamin D 250",
    dose_unit: "capsule",
    amounts: [{ key: "vitamin_d", value: 250, unit: "µg" }],
    request_id: crypto.randomUUID(),
  })

  const { supplement } = z
    .object({ supplement: z.object({ id: z.string() }) })
    .parse(JSON.parse(added))

  // Nine days of 300 g synthetic Skyr (360 mg calcium, 0 µg vitamin D), skipping a week ago; a
  // 250 µg vitamin D capsule ten and nine days ago.
  for (let daysAgo = 12; daysAgo >= 3; daysAgo -= 1) {
    if (daysAgo === 7) continue
    await send("POST", "/api/nutrition/meal-logs", {
      eaten_at: `${shiftDay(today, -daysAgo)}T11:00:00Z`,
      portions: [{ food_id: 1693, grams: 300 }],
      supplement_doses:
        daysAgo === 10 || daysAgo === 9
          ? [{ supplement_id: supplement.id, doses: 1 }]
          : [],
      request_id: crypto.randomUUID(),
    })
  }
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

async function render(data: NutritionSummaryState): Promise<void> {
  const router = createRouter({
    routeTree: createRootRoute(),
    history: createMemoryHistory({ initialEntries: ["/food?view=trends"] }),
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

function row(key: string): Element {
  const element = container.querySelector(
    `.trend-coverage-rows li[data-key="${key}"]`
  )

  expect(element, key).not.toBeNull()

  return element ?? container
}

/** The coverage table's row for a nutrient, cell by cell. */
function tableRow(name: string): string[] {
  const rows = [...container.querySelectorAll(".trend-coverage table tr")].map(
    (tr) => [...tr.children].map((cell) => normalize(cell.textContent))
  )

  return rows.find((cells) => cells[0] === name) ?? []
}

test("shows each nutrient's coverage with lasting shortfalls and cut down periods", async () => {
  const data = await readNutritionSummary({ range: 30, by: "day" })
  expect(data.state).toBe("available")
  await render(data)

  expect(text(".trend-coverage h2")).toEqual(["Vitamins and minerals"])
  expect(text(".trend-coverage .trend-summary")).toEqual([
    "Lasting shortfall: Calcium · Cut down: Vitamin D",
  ])
  expect(text(".trend-coverage-basis")[0]).toMatch(
    /over 9 logged days · a lasting shortfall is below 70 % over at least 7 logged days/
  )
  expect(text(".trend-coverage-group h3")).toEqual([
    "Vitamins",
    "Minerals",
    "Omega-3",
  ])

  // 360 of 950 mg a logged day, over nine logged days: a lasting shortfall.
  const calcium = row("calcium")
  expect(calcium.hasAttribute("data-shortfall")).toBe(true)
  expect(
    normalize(calcium.querySelector(".trend-coverage-percent")?.textContent)
  ).toBe("37 % Short")

  // Two capsule days above 100 µg; nine days ago the 7-day average of 125 µg says cut down.
  const vitaminD = row("vitamin_d")
  expect(vitaminD.getAttribute("data-limit")).toBe("cut_down")
  expect(vitaminD.hasAttribute("data-shortfall")).toBe(false)
  expect(
    normalize(vitaminD.querySelector(".trend-coverage-limit")?.textContent)
  ).toBe("Cut down in 1 day · 2 days above the Upper Limit")
  const cells = vitaminD.querySelectorAll(".trend-coverage-strip > span")
  expect(cells).toHaveLength(30)
  expect(vitaminD.querySelectorAll('[data-limit="cut_down"]')).toHaveLength(1)
  expect(vitaminD.querySelectorAll('[data-limit="above"]')).toHaveLength(1)
  // Days without Meal Logs stay empty: 21 of the 30.
  expect(vitaminD.querySelectorAll("[data-gap]")).toHaveLength(21)
  expect(
    container
      .querySelector(".trend-coverage-strip")
      ?.getAttribute("aria-hidden")
  ).toBe("true")

  // Assistive technology reads every nutrient from the table.
  expect(tableRow("Calcium")).toEqual([
    "Calcium",
    "37 % over 9 logged days",
    "Yes",
    "0",
    "None",
  ])
  const vitaminDRow = tableRow("Vitamin D")
  expect(vitaminDRow.slice(0, 4)).toEqual([
    "Vitamin D",
    "555 % over 9 logged days",
    "No",
    "2",
  ])
  expect(vitaminDRow[4]).toMatch(/^\w{3} \d{1,2} \w{3}$/)
  expect(tableRow("Vitamin C")).toEqual([
    "Vitamin C",
    "Unknown",
    "No",
    "No Upper Limit",
    "No Upper Limit",
  ])
})

test("without the Profile's sex and birth date the coverage says why and the trends stay", async () => {
  const bare = await startAxumFixture()

  try {
    vi.stubEnv("OPENHOME_API_URL", bare.url)
    const data = await readNutritionSummary({ range: 30, by: "day" })
    expect(data.state).toBe("available")
    await render(data)

    expect(text(".trend-coverage h2")).toEqual([
      "No vitamin and mineral coverage yet",
    ])
    expect(text('.trend-coverage[data-state="unavailable"] p')).toEqual([
      "The micronutrient panel needs the Profile's sex (male or female) and birth date.",
      "Sex and birth date are set on the Workout page's Body view.",
    ])
    expect(text(".trend-chart h2")).toContain("Calories")
  } finally {
    await bare.stop()
  }
})
