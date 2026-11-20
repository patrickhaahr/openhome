import { createServerFn } from "@tanstack/react-start"
import { setResponseHeader } from "@tanstack/react-start/server"
import {
  importantDateCommand,
  type ImportantDatesState,
} from "./important-dates"

export const getImportantDates = createServerFn({ method: "GET" }).handler(
  async () => {
    setResponseHeader("Cache-Control", "no-store")
    const { readImportantDates } = await import("./important-dates.server")

    return readImportantDates()
  }
)

export const mutateImportantDate = createServerFn({ method: "POST" })
  .validator(importantDateCommand)
  .handler(async ({ data }) => {
    setResponseHeader("Cache-Control", "no-store")
    const { writeImportantDate } = await import("./important-dates.server")

    return writeImportantDate(data)
  })

export async function loadImportantDates(): Promise<ImportantDatesState> {
  try {
    return await getImportantDates()
  } catch {
    return { state: "unavailable", reason: "unreachable" }
  }
}

export const getIdleDates = createServerFn({ method: "GET" }).handler(
  async () => {
    setResponseHeader("Cache-Control", "no-store")
    const { readIdleDates } = await import("./important-dates.server")

    return readIdleDates()
  }
)
