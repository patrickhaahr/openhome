import "@tanstack/react-start/server-only"
import { z } from "zod"
import { requestOpenHome } from "./openhome.server"
import { householdUser } from "./wishlist"
import {
  importantDate,
  importantDates,
  type ImportantDateCommand,
  type ImportantDateFailure,
  type ImportantDateMutation,
  type ImportantDatesState,
} from "./important-dates"

function reason(status: number): ImportantDateFailure {
  if (status === 400 || status === 422) return "invalid"

  if (status === 404) return "not-found"

  if (status === 409) return "conflict"

  return "unavailable"
}

async function read<T>(
  path: `/api/${string}`,
  schema: z.ZodType<T>,
  query?: Record<string, string>
): Promise<
  | { state: "read"; data: T }
  | { state: "unavailable"; reason: ImportantDateFailure }
> {
  const result = await requestOpenHome({
    path,
    query,
    method: "GET",
    timeoutMs: 5000,
  })

  if (result.state !== "received")
    return { state: "unavailable", reason: result.state }

  if (!result.response.ok)
    return { state: "unavailable", reason: reason(result.response.status) }

  try {
    const parsed = schema.safeParse(await result.response.json())

    if (parsed.success) return { state: "read", data: parsed.data }
  } catch {
    /* Parse only safe protocol values. */
  }

  return { state: "unavailable", reason: "unavailable" }
}

/** Read the household and today's figures in parallel through the server-held credential. */
export async function readImportantDates(): Promise<ImportantDatesState> {
  const [owners, dates] = await Promise.all([
    read("/api/household-users", householdUser.array()),
    read("/api/important-dates", importantDates),
  ])

  if (owners.state !== "read") return owners

  if (dates.state !== "read") return dates

  return {
    state: "available",
    owners: owners.data,
    on: dates.data.on,
    dates: dates.data.dates,
  }
}

/** Read only the dates explicitly allowed on the shared room display. */
export async function readIdleDates(): Promise<ImportantDatesState> {
  const dates = await read("/api/important-dates", importantDates, {
    show_when_idle: "true",
  })

  if (dates.state !== "read") return dates

  return {
    state: "available",
    owners: [],
    on: dates.data.on,
    dates: dates.data.dates,
  }
}

/** Forward one operation; only an API receipt confirms persistence. */
export async function writeImportantDate(
  command: ImportantDateCommand
): Promise<ImportantDateMutation> {
  let request: Omit<Parameters<typeof requestOpenHome>[0], "timeoutMs">

  switch (command.action) {
    case "add":
      request = {
        path: "/api/important-dates",
        method: "POST",
        json: {
          title: command.title,
          date: command.date,
          kind: command.kind,
          person_ids: command.person_ids,
          show_when_idle: command.show_when_idle,
          request_id: command.request_id,
        },
      }
      break
    case "edit":
      request = {
        path: `/api/important-dates/${command.date_id}`,
        method: "PATCH",
        json: {
          title: command.title,
          date: command.date,
          kind: command.kind,
          person_ids: command.person_ids,
          show_when_idle: command.show_when_idle,
        },
      }
      break
    case "remove":
      request = {
        path: `/api/important-dates/${command.date_id}`,
        method: "DELETE",
      }
      break
  }

  const result = await requestOpenHome({ ...request, timeoutMs: 5000 })

  if (result.state !== "received")
    return { state: "failed", reason: result.state }

  if (!result.response.ok)
    return { state: "failed", reason: reason(result.response.status) }

  if (command.action === "remove")
    return result.response.status === 204
      ? { state: "saved" }
      : { state: "failed", reason: "unavailable" }

  try {
    const body: unknown = await result.response.json()

    const schema =
      command.action === "add"
        ? z.object({ important_date: importantDate, replayed: z.boolean() })
        : importantDate

    if (schema.safeParse(body).success) return { state: "saved" }
  } catch {
    /* An invalid acknowledgement cannot confirm a saved date. */
  }

  return { state: "failed", reason: "unavailable" }
}
