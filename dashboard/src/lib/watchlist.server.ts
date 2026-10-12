import "@tanstack/react-start/server-only"
import { z } from "zod"
import { requestOpenHome } from "./openhome.server"
import { householdUser } from "./wishlist"
import {
  watchlistEntry,
  type WatchlistCommand,
  type WatchlistFailure,
  type WatchlistMutation,
  type WatchlistState,
} from "./watchlist"

function reason(status: number): WatchlistFailure {
  if (status === 400 || status === 422) return "invalid"

  if (status === 404) return "not-found"

  if (status === 409) return "conflict"

  return "unavailable"
}

async function read<T>(
  path: `/api/${string}`,
  schema: z.ZodType<T>
): Promise<
  | { state: "read"; data: T }
  | { state: "unavailable"; reason: WatchlistFailure }
> {
  const result = await requestOpenHome({ path, method: "GET", timeoutMs: 5000 })

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

/** Read owners and all plans in parallel through the server-held credential. */
export async function readWatchlists(): Promise<WatchlistState> {
  const [owners, entries] = await Promise.all([
    read("/api/household-users", householdUser.array()),
    read("/api/watchlist/entries", watchlistEntry.array()),
  ])

  if (owners.state !== "read") return owners

  if (entries.state !== "read") return entries

  return { state: "available", owners: owners.data, entries: entries.data }
}

/** Forward one owner-targeted operation; only an API receipt confirms persistence. */
export async function writeWatchlist(
  command: WatchlistCommand
): Promise<WatchlistMutation> {
  const request =
    command.action === "add"
      ? {
          path: "/api/watchlist/entries" as const,
          method: "POST" as const,
          json: {
            owner_id: command.owner_id,
            kind: command.kind,
            title: command.title,
            request_id: command.request_id,
          },
        }
      : {
          path: `/api/household-users/${command.owner_id}/watchlist/entries/${command.entry_id}` as const,
          method: "DELETE" as const,
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
    if (
      z
        .object({ entry: watchlistEntry, replayed: z.boolean() })
        .safeParse(await result.response.json()).success
    )
      return { state: "saved" }
  } catch {
    /* An invalid acknowledgement cannot confirm a saved plan. */
  }

  return { state: "failed", reason: "unavailable" }
}
