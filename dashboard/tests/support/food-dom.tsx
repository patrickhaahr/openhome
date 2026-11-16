import { act } from "react"
import {
  createMemoryHistory,
  createRootRoute,
  createRouter,
  RouterContextProvider,
} from "@tanstack/react-router"
import { deflateSync } from "node:zlib"
import { createRoot, type Root } from "react-dom/client"
import { afterAll, afterEach, beforeAll, beforeEach, expect, vi } from "vitest"
import { z } from "zod"
import { FoodDay } from "@/components/food-day"
import type { NutritionDayState } from "@/lib/nutrition"
import { readNutritionDay, writeNutritionGoal } from "@/lib/nutrition.server"
import { startAxumFixture } from "./axum-fixture"

export const apiKey = "synthetic-dashboard-api-key"

export let ready: Awaited<ReturnType<typeof startAxumFixture>>

export let importing: Awaited<ReturnType<typeof startAxumFixture>>

export let root: Root

export let container: HTMLDivElement

export const logged: string[] = []

export const savedMeals: string[] = []

export const addedSupplements: string[] = []

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
export function syntheticPng(width: number, height: number): string {
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
export async function logMeal(meal: {
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

export let refreshWork: Promise<void> = Promise.resolve()

export let goalWork: Promise<unknown> = Promise.resolve()

/** Renders the Day view; after a goal change it reads the same day again, as the page does. */
export async function render(data: NutritionDayState): Promise<void> {
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

export function normalize(value: string | null | undefined): string {
  return value?.replace(/\s+/g, " ").trim() ?? ""
}

export function text(selector: string): string[] {
  return [...container.querySelectorAll(selector)].map((element) =>
    normalize(element.textContent)
  )
}

/** Each match's child elements' text, joined by spaces: a row as it reads across. */
export function rows(selector: string): string[] {
  return [...container.querySelectorAll(selector)].map((element) =>
    [...element.children].map((child) => normalize(child.textContent)).join(" ")
  )
}

/** Sends an authenticated JSON write to an Axum fixture, as the agent's tools do. */
export async function write(
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
