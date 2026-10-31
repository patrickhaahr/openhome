import { createServerFn } from "@tanstack/react-start"
import { setResponseHeader } from "@tanstack/react-start/server"
import { trainingDayOn } from "./workout"
import {
  nutritionSearch,
  setGoalInput,
  type NutritionDayState,
  type NutritionSearch,
} from "./nutrition"

/** Read one Nutrition Day using the server-held Axum credential. */
export const getNutritionDay = createServerFn({ method: "GET" })
  .validator(nutritionSearch)
  .handler(async ({ data }) => {
    setResponseHeader("Cache-Control", "no-store")
    const { readNutritionDay } = await import("./nutrition.server")

    return readNutritionDay(data)
  })

/** Set the Nutrition Goal from today, with the credential kept on the server. */
export const saveNutritionGoal = createServerFn({ method: "POST" })
  .validator(setGoalInput)
  .handler(async ({ data }) => {
    setResponseHeader("Cache-Control", "no-store")
    const { writeNutritionGoal } = await import("./nutrition.server")

    return writeNutritionGoal(data)
  })

/**
 * Read the Day view; a dropped read RPC renders an unavailable day instead of a route error.
 *
 * @param search - The day to show; today when absent.
 * @returns The day's read, or an unreachable outcome.
 */
export async function loadNutritionDay(
  search: NutritionSearch
): Promise<NutritionDayState> {
  try {
    return await getNutritionDay({ data: search })
  } catch {
    const today = trainingDayOn(new Date())

    return {
      day: search.day ?? today,
      today,
      state: "unavailable",
      reason: "unreachable",
    }
  }
}
