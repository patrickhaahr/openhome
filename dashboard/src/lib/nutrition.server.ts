import "@tanstack/react-start/server-only"
import { z } from "zod"
import { readBoundedBody, requestOpenHome } from "./openhome.server"
import { trainingDayOn } from "./workout"
import {
  foodDatabaseUnavailable,
  nutritionDay,
  type NutritionDayState,
  type NutritionFailure,
  type NutritionSearch,
} from "./nutrition"

function reason(status: number): NutritionFailure {
  if (status === 400 || status === 404 || status === 422) return "invalid"

  return "unavailable"
}

/**
 * Read one Copenhagen day's Meal Logs and totals through the server-held Axum credential.
 *
 * @param search - The day to read; today when absent.
 * @returns The day, the Food database's unavailability, or a sanitized failure.
 */
export async function readNutritionDay(
  search: NutritionSearch
): Promise<NutritionDayState> {
  const today = trainingDayOn(new Date())
  const day = search.day ?? today

  const result = await requestOpenHome({
    path: `/api/nutrition/days/${day}`,
    method: "GET",
    timeoutMs: 5000,
  })

  if (result.state !== "received")
    return { day, today, state: "unavailable", reason: result.state }
  const { response } = result
  const body: unknown = await response.json().catch(() => undefined)

  if (response.status === 503) {
    const unavailable = foodDatabaseUnavailable.safeParse(body)

    if (unavailable.success)
      return {
        day,
        today,
        state: "food-database-unavailable",
        database: unavailable.data.state,
      }
  }

  if (!response.ok)
    return { day, today, state: "unavailable", reason: reason(response.status) }
  const parsed = nutritionDay.safeParse(body)

  return parsed.success
    ? { day, today, state: "available", data: parsed.data }
    : { day, today, state: "unavailable", reason: "unavailable" }
}

const mealPhotoInput = z.object({
  photo: z.uuid(),
  size: z.enum(["photo", "thumbnail"]),
})

// A 1,600 px AVIF is a few hundred kilobytes; anything far larger is not one of Axum's photos.
const mealPhotoLimit = 5 * 1024 * 1024

/**
 * Relay one stored meal photo or thumbnail as bounded AVIF bytes, never upstream headers or
 * error bodies, the way wish pictures are relayed.
 *
 * @param input - The photo ID and size from the dashboard path.
 * @returns The image, or an empty 404 or 503.
 */
export async function readMealPhoto(input: {
  photo: string
  size: string
}): Promise<Response> {
  const headers = {
    // A photo ID always names the same image; a replaced photo gets a new ID.
    "Cache-Control": "private, max-age=31536000, immutable",
    "X-Content-Type-Options": "nosniff",
    "Content-Security-Policy":
      "default-src 'none'; style-src 'unsafe-inline'; sandbox",
  }

  const failed = (status: number): Response =>
    new Response(null, {
      status,
      headers: { ...headers, "Cache-Control": "no-store" },
    })

  const parsed = mealPhotoInput.safeParse(input)

  if (!parsed.success) return failed(404)
  const { photo, size } = parsed.data

  const result = await requestOpenHome({
    path:
      size === "photo"
        ? `/api/nutrition/meal-photos/${photo}`
        : `/api/nutrition/meal-photos/${photo}/thumbnail`,
    method: "GET",
    timeoutMs: 12000,
  })

  if (result.state !== "received") return failed(503)
  const response = result.response

  if (!response.ok || response.headers.get("content-type") !== "image/avif") {
    await response.body?.cancel()

    return failed(response.status === 404 ? 404 : 503)
  }

  const body = await readBoundedBody(response, mealPhotoLimit)

  if (body === undefined) return failed(503)

  return new Response(body, {
    headers: { ...headers, "Content-Type": "image/avif" },
  })
}
