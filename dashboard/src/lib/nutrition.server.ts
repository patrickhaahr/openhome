import "@tanstack/react-start/server-only"
import { requestOpenHome } from "./openhome.server"
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
