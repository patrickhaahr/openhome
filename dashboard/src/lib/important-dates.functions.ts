import { createServerFn } from "@tanstack/react-start"
import { setResponseHeader } from "@tanstack/react-start/server"
import {
  importantDateCommand,
  type ImportantDatesState,
} from "./important-dates"

/** Household-shared read through authenticated Axum, with credentials held by Start. */
export const getImportantDates = createServerFn({ method: "GET" }).handler(
  async () => {
    setResponseHeader("Cache-Control", "no-store")
    const { readImportantDates } = await import("./important-dates.server")

    return readImportantDates()
  }
)

/** Writes use the same Axum rules and receipts as MCP. */
export const mutateImportantDate = createServerFn({ method: "POST" })
  .validator(importantDateCommand)
  .handler(async ({ data }) => {
    setResponseHeader("Cache-Control", "no-store")
    const { writeImportantDate } = await import("./important-dates.server")

    return writeImportantDate(data)
  })

/** A dropped RPC keeps recovery reachable within the focused view. */
export async function loadImportantDates(): Promise<ImportantDatesState> {
  try {
    return await getImportantDates()
  } catch {
    return { state: "unavailable", reason: "unreachable" }
  }
}

/** A privacy-filtered read for the shared idle display; credentials stay on the server. */
export const getIdleDates = createServerFn({ method: "GET" }).handler(
  async () => {
    setResponseHeader("Cache-Control", "no-store")
    const { readIdleDates } = await import("./important-dates.server")

    return readIdleDates()
  }
)
