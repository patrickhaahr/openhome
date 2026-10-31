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
import { deflateSync } from "node:zlib"
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
import {
  mealTime,
  shiftDay,
  type DoseUnit,
  type NutritionDayState,
} from "@/lib/nutrition"
import {
  readMealPhoto,
  readNutritionDay,
  writeNutritionGoal,
} from "@/lib/nutrition.server"
import { trainingDayOn } from "@/lib/workout"
import { startAxumFixture } from "./support/axum-fixture"

const apiKey = "synthetic-dashboard-api-key"

let ready: Awaited<ReturnType<typeof startAxumFixture>>

let importing: Awaited<ReturnType<typeof startAxumFixture>>

let root: Root

let container: HTMLDivElement

const logged: string[] = []

const savedMeals: string[] = []

const addedSupplements: string[] = []

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

  for (const id of savedMeals.splice(0))
    await fetch(`${ready.url}/api/nutrition/meals/${id}`, {
      method: "DELETE",
      headers: { Authorization: `Bearer ${apiKey}` },
    })

  for (const id of addedSupplements.splice(0))
    await fetch(`${ready.url}/api/nutrition/supplements/${id}`, {
      method: "DELETE",
      headers: { Authorization: `Bearer ${apiKey}` },
    })
  vi.unstubAllEnvs()
  vi.unstubAllGlobals()
})

const crcTable = Array.from({ length: 256 }, (_, index) => {
  let crc = index

  for (let bit = 0; bit < 8; bit++)
    crc = crc & 1 ? 0xedb88320 ^ (crc >>> 1) : crc >>> 1

  return crc >>> 0
})

function crc32(bytes: Uint8Array): number {
  let crc = 0xffffffff

  for (const byte of bytes)
    crc = (crcTable[(crc ^ byte) & 0xff] ?? 0) ^ (crc >>> 8)

  return (crc ^ 0xffffffff) >>> 0
}

/** A synthetic RGB PNG of a gradient plate, base64-encoded as the agent sends photos. */
function syntheticPng(width: number, height: number): string {
  const chunk = (type: string, data: Uint8Array): Buffer => {
    const body = Buffer.concat([Buffer.from(type, "latin1"), data])
    const framed = Buffer.alloc(body.length + 8)
    framed.writeUInt32BE(data.length, 0)
    body.copy(framed, 4)
    framed.writeUInt32BE(crc32(body), body.length + 4)

    return framed
  }

  const header = Buffer.alloc(13)
  header.writeUInt32BE(width, 0)
  header.writeUInt32BE(height, 4)
  header.set([8, 2, 0, 0, 0], 8)
  const rows = Buffer.alloc((width * 3 + 1) * height)

  for (let y = 0; y < height; y++)
    for (let x = 0; x < width; x++)
      rows.set(
        [(x * 255) / width, 180, (y * 255) / height],
        y * (width * 3 + 1) + 1 + x * 3
      )

  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk("IHDR", header),
    chunk("IDAT", deflateSync(rows)),
    chunk("IEND", new Uint8Array()),
  ]).toString("base64")
}

/** Logs a synthetic meal through the API, as the agent does, and returns its day. */
async function logMeal(meal: {
  api?: string
  eatenAt: Date
  note?: string
  photo?: string
  portions: Array<
    { food_id: number; grams: number } | { barcode: string; grams: number }
  >
}): Promise<{ id: string; day: string; eaten_at: string }> {
  const response = await fetch(
    `${meal.api ?? ready.url}/api/nutrition/meal-logs`,
    {
      method: "POST",
      headers: {
        Authorization: `Bearer ${apiKey}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify({
        eaten_at: meal.eatenAt.toISOString(),
        note: meal.note,
        photo: meal.photo,
        portions: meal.portions,
        request_id: crypto.randomUUID(),
      }),
    }
  )

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

let refreshWork: Promise<void> = Promise.resolve()

let goalWork: Promise<unknown> = Promise.resolve()

/** Renders the Day view; after a goal change it reads the same day again, as the page does. */
async function render(data: NutritionDayState): Promise<void> {
  const router = createRouter({
    routeTree: createRootRoute(),
    history: createMemoryHistory(),
    isServer: false,
  })

  await act(async () =>
    root.render(
      <RouterContextProvider router={router}>
        <FoodDay
          data={data}
          saveGoal={(goal) => {
            const work = writeNutritionGoal(goal)
            goalWork = work

            return work
          }}
          reload={() => {
            refreshWork = readNutritionDay({ day: data.day }).then(render)

            return refreshWork
          }}
        />
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

  const portions = [...container.querySelectorAll(".food-meal")]
    .find(
      (element) =>
        element.querySelector("time")?.textContent === mealTime(meal.eaten_at)
    )
    ?.querySelectorAll(".food-portions li")

  // The label's 76 kcal per 100 g, not the linked Skyr's 62.
  expect(
    [...(portions ?? [])].map((row) =>
      [...row.children].map((child) => normalize(child.textContent)).join(" ")
    )
  ).toEqual([
    "Vanilla Skyr, NOVA 3, Synthetic Dairy 150 g 114 kcal",
    "Oat flakes 40 g 148 kcal",
  ])
  expect(text(".food-portion-brand")).toContain(", Synthetic Dairy")
  expect(text(".food-credit")).toEqual([
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
    [...container.querySelectorAll(".food-meal")].find(
      (element) =>
        element.querySelector("time")?.textContent === mealTime(eatenAt)
    )

  expect(
    normalize(
      plate(fromMeal.eaten_at)?.querySelector(".food-meal-from")?.textContent
    )
  ).toBe("Logged from the Meal Protein oats")
  expect(
    [
      ...(plate(fromMeal.eaten_at)?.querySelectorAll(".food-portions li") ??
        []),
    ].map((row) =>
      [...row.children].map((child) => normalize(child.textContent)).join(" ")
    )
  ).toEqual(["Skyr, 0.2 % fat 150 g 93 kcal", "Oat flakes 70 g 259 kcal"])

  if (oneOff.day === fromMeal.day)
    expect(plate(oneOff.eaten_at)?.querySelector(".food-meal-from")).toBeNull()
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
    normalize(container.querySelector(".food-supplements h3")?.textContent)
  ).toBe("Supplements")
  expect(rows(".food-supplements li")).toEqual([
    "Fish oil 2 capsules",
    "Vitamin D3 1 capsule",
    "Creatine 1 scoop",
    "Creatine monohydrate 3 g",
  ])
  expect(rows(".food-meal .food-doses li").map((row) => row.trim())).toEqual([
    "Fish oil 2 capsules 18 kcal",
    "Vitamin D3 1 capsule",
    "Creatine 1 scoop",
  ])
  // The skyr's 93 kcal and 0.3 g fat, plus what the fish oil's label states.
  expect(text(".food-meal header .food-amount")).toContain("111 kcal")
  expect(text(".food-energy-supplements")).toEqual(["18 kcal from supplements"])
  expect(rows('.food-macros > div[data-macro="fat"]')).toEqual([
    "Fat 2.3 g 2 g from supplements",
  ])

  await act(async () => {
    const button = [...container.querySelectorAll("button")].find(
      (element) => element.textContent === "All nutrients"
    )

    button?.click()
  })
  expect(rows(".food-nutrient-groups dl div")).toContain(
    "Vitamin D 25 µg Food 0 µg · supplements 25 µg"
  )
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

test("a meal's photo shows as a thumbnail, opens in full on selection and is relayed as AVIF", async () => {
  const meal = await logMeal({
    eatenAt: new Date(Date.now() - 90_000),
    photo: syntheticPng(800, 600),
    portions: [{ food_id: 300, grams: 200 }],
  })

  await render(await readNutritionDay({ day: meal.day }))

  const thumbnail = [...container.querySelectorAll(".food-meal")]
    .find(
      (element) =>
        element.querySelector("time")?.textContent === mealTime(meal.eaten_at)
    )
    ?.querySelector<HTMLButtonElement>("button.food-photo")

  expect(thumbnail?.getAttribute("aria-label")).toBe(
    `Show the photo of the meal at ${mealTime(meal.eaten_at)}`
  )
  const small = thumbnail?.querySelector("img")
  const photoId = small?.getAttribute("src")?.split("/")[2] ?? ""
  expect(small?.getAttribute("src")).toBe(`/meal-photo/${photoId}/thumbnail`)
  expect([small?.getAttribute("width"), small?.getAttribute("height")]).toEqual(
    ["400", "300"]
  )
  expect(document.querySelector(".food-photo-viewer")).toBeNull()

  await act(async () => thumbnail?.click())
  const full = document.querySelector(".food-photo-viewer img")
  expect(full?.getAttribute("src")).toBe(`/meal-photo/${photoId}/photo`)
  expect(full?.getAttribute("alt")).toBe(
    `The meal at ${mealTime(meal.eaten_at)}`
  )
  expect([full?.getAttribute("width"), full?.getAttribute("height")]).toEqual([
    "800",
    "600",
  ])

  // The relay holds the API key server-side and passes only the AVIF bytes on.
  for (const size of ["thumbnail", "photo"]) {
    const relayed = await readMealPhoto({ photo: photoId, size })
    expect(relayed.status).toBe(200)
    expect(relayed.headers.get("content-type")).toBe("image/avif")
    expect(relayed.headers.get("cache-control")).toBe(
      "private, max-age=31536000, immutable"
    )
    const bytes = new Uint8Array(await relayed.arrayBuffer())
    expect(new TextDecoder().decode(bytes.subarray(4, 12))).toBe("ftypavif")
  }

  for (const input of [
    { photo: photoId, size: "original" },
    { photo: "not-an-id", size: "photo" },
    { photo: crypto.randomUUID(), size: "thumbnail" },
  ]) {
    const missing = await readMealPhoto(input)
    expect(missing.status).toBe(404)
    expect(missing.headers.get("cache-control")).toBe("no-store")
  }
})

/** Sends an authenticated JSON write to an Axum fixture, as the agent's tools do. */
async function write(
  api: string,
  method: "POST" | "PUT" | "PATCH",
  path: string,
  body: Readonly<Record<string, string | number>>
): Promise<number> {
  const response = await fetch(`${api}${path}`, {
    method,
    headers: {
      Authorization: `Bearer ${apiKey}`,
      "Content-Type": "application/json",
    },
    body: JSON.stringify(body),
  })

  return response.status
}

test("the day shows its NOVA 1 share of classified energy and each Portion's NOVA Class", async () => {
  // Classes outlive Meal Logs, so this day gets a fixture of its own.
  const nova = await startAxumFixture()

  try {
    vi.stubEnv("OPENHOME_API_URL", nova.url)
    const empty = await readNutritionDay({ day: "2026-01-15" })
    await render(empty)
    expect(rows(".food-nova")).toEqual([
      "Unknown NOVA 1 Nothing classified yet",
    ])

    const barcode = "2001234567893"
    expect(
      await write(nova.url, "POST", "/api/nutrition/products", {
        barcode,
        food_id: 1693,
        request_id: "dashboard-nova-link",
      })
    ).toBe(201)

    for (const food of [3, 907])
      expect(
        await write(
          nova.url,
          "PUT",
          `/api/nutrition/foods/${food}/nova-class`,
          {
            nova_class: 1,
          }
        )
      ).toBe(200)

    const meal = await logMeal({
      api: nova.url,
      eatenAt: new Date(Date.now() - 60_000),
      portions: [
        { food_id: 3, grams: 100 },
        { food_id: 907, grams: 100 },
        { barcode, grams: 100 },
        { food_id: 300, grams: 100 },
      ],
    })

    await render(await readNutritionDay({ day: meal.day }))

    // Banana 90 and chicken 110 kcal are NOVA 1 of 276 classified kcal; the skyr Product is
    // Open Food Facts' Class 3 and the rice is left out unclassified.
    expect(rows(".food-nova")).toEqual([
      "72 % NOVA 1 Unprocessed or minimally processed, of 276 classified kcal · 1 unclassified portion left out",
    ])
    expect(text(".food-portions .food-portion-nova")).toEqual([
      ", NOVA 1",
      ", NOVA 1",
      ", NOVA 3",
    ])
    expect(
      container.querySelector(".food-portion-nova")?.getAttribute("title")
    ).toBe("NOVA 1: unprocessed or minimally processed")
  } finally {
    await nova.stop()
  }
})

test("the day shows its Calorie Target, eaten and remaining with macro progress, and saves the goal from today", async () => {
  // The Profile, Body Weight and goal outlive Meal Logs, so this day gets a fixture of its own.
  const targets = await startAxumFixture()

  try {
    vi.stubEnv("OPENHOME_API_URL", targets.url)
    const today = trainingDayOn(new Date())
    await render(await readNutritionDay({ day: today }))
    expect(text(".food-targets h2")).toEqual(["No Calorie Target yet"])
    expect(text(".food-targets p")[0]).toBe(
      `Targets need the Profile's height, sex (male or female) and birth date, and a Body Weight on or before ${today}.`
    )

    // A man of 180 cm and 80 kg who turned 30 120 days ago: BMR 1,780 kcal, so with no logged
    // days the Estimated Expenditure is 1.5 × BMR and losing 0.5 kg a week takes 550 kcal off.
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
    // 1,000 g of chicken breast: 1,100 kcal, 230 g protein and 15 g fat.

    const meal = await logMeal({
      api: targets.url,
      eatenAt: new Date(Date.now() - 1000),
      portions: [{ food_id: 907, grams: 1000 }],
    })

    await render(await readNutritionDay({ day: meal.day }))

    expect(rows(".food-target-strip > div")).toEqual([
      "Target 2,120 kcal",
      "Eaten 1,100 kcal",
      "Remaining 1,020 kcal",
    ])
    expect(text(".food-target-basis")).toEqual([
      "Lose 0.5 kg a week · Estimated Expenditure 2,670 kcal · Trend Weight 80 kg",
    ])
    expect(text(".food-macro-target-name")).toEqual(["Protein", "Fat", "Carbs"])
    expect(text(".food-macro-target-amount")).toEqual([
      "230 / 176 g",
      "15 / 56 g",
      "0 / 228 g",
    ])
    expect(text(".food-macro-target-status")).toEqual([
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
    // The control reads the day again once Axum confirms the goal.
    await act(async () => {
      await vi.waitFor(() => expect(refreshWork).not.toBe(settled))
      await refreshWork
    })
    expect(text(".food-goal-feedback")).toEqual([
      "Goal saved: Maintain from today.",
    ])

    expect(rows(".food-target-strip > div")).toEqual([
      "Target 2,670 kcal",
      "Eaten 1,100 kcal",
      "Remaining 1,570 kcal",
    ])
    expect(text(".food-target-basis")[0]).toMatch(/^Maintain · /)

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
