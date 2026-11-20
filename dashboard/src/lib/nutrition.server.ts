import "@tanstack/react-start/server-only"
import { z } from "zod"
import { readBoundedBody, requestOpenHome } from "./openhome.server"
import { trainingDayOn } from "./workout"
import {
  foodDatabaseUnavailable,
  nutritionDay,
  nutritionGoal,
  type GoalMutation,
  type NutritionDayState,
  type NutritionFailure,
  type NutritionGoal,
  type NutritionSearch,
  type SetGoalInput,
  shiftDay,
} from "./nutrition"
import {
  nutritionSummary,
  type NutritionSummaryState,
  type TrendsChoice,
} from "./nutrition-trends"

function reason(status: number): NutritionFailure {
  if (status === 400 || status === 404 || status === 422) return "invalid"

  return "unavailable"
}

export async function readNutritionDay(
  search: NutritionSearch
): Promise<NutritionDayState> {
  const today = trainingDayOn(new Date())
  const day = search.day ?? today

  const [result, goal] = await Promise.all([
    requestOpenHome({
      path: `/api/nutrition/days/${day}`,
      method: "GET",
      timeoutMs: 5000,
    }),
    readNutritionGoal(),
  ])

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
    ? { day, today, state: "available", data: parsed.data, goal }
    : { day, today, state: "unavailable", reason: "unavailable" }
}

export async function readNutritionSummary(
  choice: TrendsChoice
): Promise<NutritionSummaryState> {
  const today = trainingDayOn(new Date())

  const result = await requestOpenHome({
    path: "/api/nutrition/summary",
    method: "GET",
    query: {
      from: shiftDay(today, 1 - choice.range),
      to: today,
      aggregation: choice.by,
    },
    timeoutMs: 10000,
  })

  if (result.state !== "received")
    return { ...choice, today, state: "unavailable", reason: result.state }
  const { response } = result

  if (!response.ok)
    return {
      ...choice,
      today,
      state: "unavailable",
      reason: reason(response.status),
    }
  const body: unknown = await response.json().catch(() => undefined)
  const parsed = nutritionSummary.safeParse(body)

  return parsed.success
    ? { ...choice, today, state: "available", data: parsed.data }
    : { ...choice, today, state: "unavailable", reason: "unavailable" }
}

async function readNutritionGoal(): Promise<NutritionGoal | null> {
  const result = await requestOpenHome({
    path: "/api/nutrition/goal",
    method: "GET",
    timeoutMs: 5000,
  })

  if (result.state !== "received" || !result.response.ok) return null
  const body: unknown = await result.response.json().catch(() => undefined)
  const parsed = nutritionGoal.safeParse(body)

  return parsed.success ? parsed.data : null
}

export async function writeNutritionGoal(
  input: SetGoalInput
): Promise<GoalMutation> {
  const result = await requestOpenHome({
    path: "/api/nutrition/goal",
    method: "PUT",
    json: input,
    timeoutMs: 5000,
  })

  if (result.state !== "received")
    return { state: "failed", reason: result.state }
  const { response } = result

  if (!response.ok) return { state: "failed", reason: reason(response.status) }
  const body: unknown = await response.json().catch(() => undefined)
  const parsed = nutritionGoal.safeParse(body)

  return parsed.success
    ? { state: "saved", goal: parsed.data }
    : { state: "failed", reason: "unavailable" }
}

const mealPhotoInput = z.object({
  photo: z.uuid(),
  size: z.enum(["photo", "thumbnail"]),
})

const mealPhotoLimit = 5 * 1024 * 1024

export async function readMealPhoto(input: {
  photo: string
  size: string
}): Promise<Response> {
  const headers = {
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
