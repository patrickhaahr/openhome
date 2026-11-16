// @vitest-environment happy-dom
// @vitest-environment-options {"settings":{"fetch":{"disableSameOriginPolicy":true}}}
// The private server transport calls Axum; browser CORS applies only to Start RPCs.
import { act } from "react"
import { expect, test, vi } from "vitest"
import { mealTime, shiftDay } from "@/lib/nutrition"
import { readMealPhoto, readNutritionDay } from "@/lib/nutrition.server"
import { trainingDayOn } from "@/lib/workout"
import { startAxumFixture } from "./support/axum-fixture"
import {
  container,
  importing,
  logMeal,
  render,
  rows,
  syntheticPng,
  text,
  write,
} from "./support/food-dom"

test("a backfilled meal shows on its own earlier day, which can step forward again", async () => {
  const yesterday = await logMeal({
    eatenAt: new Date(Date.now() - 26 * 60 * 60 * 1000),
    portions: [{ food_id: 907, grams: 200 }],
  })

  const today = trainingDayOn(new Date())

  await render(await readNutritionDay({ day: yesterday.day }))

  expect(rows("[data-slot='food-portions'] li")).toEqual([
    "Chicken, breast, flesh, raw 200 g 220 kcal",
  ])
  expect(text("[data-slot='food-day-name']")[0]).not.toBe("Today")
  expect(
    container.querySelector('a[aria-label^="Next day"]')?.getAttribute("href")
  ).toBe(
    shiftDay(yesterday.day, 1) === today
      ? "/food"
      : `/food?day=${shiftDay(yesterday.day, 1)}`
  )
  expect(text("[data-slot='food-today']")).toEqual(["Today"])
})

test("an empty day says nothing was logged and leaves unknown totals as dashes", async () => {
  await render(await readNutritionDay({ day: "2026-01-15" }))

  expect(text("[data-slot='food-empty'] h3")).toEqual(["Nothing logged"])
  expect(text("[data-slot='food-energy'] [data-gap]")).toEqual(["Unknown"])
  expect(container.querySelector("[data-slot='food-split']")).toBeNull()
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

  expect(text("[data-slot='food-unavailable'] h2")).toEqual([
    "Food database importing",
  ])
  expect(text("[data-slot='food-day-name']")).toHaveLength(1)
  expect(text("[data-slot='food-credit']")[0]).toContain(
    "© The Danish Food Composition Database"
  )

  vi.stubEnv("OPENHOME_API_URL", "http://127.0.0.1:1")
  await render(await readNutritionDay({ day: "2026-01-15" }))
  expect(text("[data-slot='food-unavailable'] h2")).toEqual([
    "Food unavailable",
  ])
})

test("a meal's photo shows as a thumbnail, opens in full on selection and is relayed as AVIF", async () => {
  const meal = await logMeal({
    eatenAt: new Date(Date.now() - 90_000),
    photo: syntheticPng(800, 600),
    portions: [{ food_id: 300, grams: 200 }],
  })

  await render(await readNutritionDay({ day: meal.day }))

  const thumbnail = [...container.querySelectorAll("[data-slot='food-meal']")]
    .find(
      (element) =>
        element.querySelector("time")?.textContent === mealTime(meal.eaten_at)
    )
    ?.querySelector<HTMLButtonElement>("button[data-slot='food-photo']")

  expect(thumbnail?.getAttribute("aria-label")).toBe(
    `Show the photo of the meal at ${mealTime(meal.eaten_at)}`
  )
  const small = thumbnail?.querySelector("img")
  const photoId = small?.getAttribute("src")?.split("/")[2] ?? ""
  expect(small?.getAttribute("src")).toBe(`/meal-photo/${photoId}/thumbnail`)
  expect([small?.getAttribute("width"), small?.getAttribute("height")]).toEqual(
    ["400", "300"]
  )
  expect(document.querySelector("[data-slot='food-photo-viewer']")).toBeNull()

  await act(async () => thumbnail?.click())
  const full = document.querySelector("[data-slot='food-photo-viewer'] img")
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

test("the day shows its NOVA 1 share of classified energy and each Portion's NOVA Class", async () => {
  // Classes outlive Meal Logs, so this day gets a fixture of its own.
  const nova = await startAxumFixture()

  try {
    vi.stubEnv("OPENHOME_API_URL", nova.url)
    const empty = await readNutritionDay({ day: "2026-01-15" })
    await render(empty)
    expect(rows("[data-slot='food-nova']")).toEqual([
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
    expect(rows("[data-slot='food-nova']")).toEqual([
      "72 % NOVA 1 Unprocessed or minimally processed, of 276 classified kcal · 1 unclassified portion left out",
    ])
    expect(
      text("[data-slot='food-portions'] [data-slot='food-portion-nova']")
    ).toEqual([", NOVA 1", ", NOVA 1", ", NOVA 3"])
    expect(
      container
        .querySelector("[data-slot='food-portion-nova']")
        ?.getAttribute("title")
    ).toBe("NOVA 1: unprocessed or minimally processed")
  } finally {
    await nova.stop()
  }
})
