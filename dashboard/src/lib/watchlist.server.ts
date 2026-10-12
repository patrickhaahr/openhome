import "@tanstack/react-start/server-only"
import { z } from "zod"
import { requestOpenHome } from "./openhome.server"
import { householdUser } from "./wishlist"
import {
  watchlistEntry,
  mediaMetadata,
  mediaSearch,
  type MediaSearchResult,
  type WatchlistCommand,
  type WatchlistFailure,
  type WatchlistMutation,
  type WatchlistState,
} from "./watchlist"

function reason(status: number): WatchlistFailure {
  if (status === 400 || status === 422) return "invalid"

  if (status === 404) return "not-found"

  if (status === 503) return "tmdb-unavailable"

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
  const path =
    `/api/household-users/${command.owner_id}/watchlist/entries/${command.action === "add" ? "" : command.entry_id}` as const

  let request: Omit<Parameters<typeof requestOpenHome>[0], "timeoutMs">

  switch (command.action) {
    case "add":
      request = {
        path: "/api/watchlist/entries",
        method: "POST",
        json: {
          owner_id: command.owner_id,
          kind: command.kind,
          title: command.title,
          request_id: command.request_id,
          tmdb_id: command.tmdb_id,
        },
      }
      break
    case "edit":
      request = {
        path,
        method: "PATCH",
        json: {
          status: command.status,
          date_added: command.date_added,
          completion_date: command.completion_date,
        },
      }
      break
    case "enrich":
      request = {
        path: `${path}/metadata`,
        method: "POST",
        json: { tmdb_id: command.tmdb_id },
      }
      break
    case "remove":
      request = { path, method: "DELETE" }
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
        ? z.object({ entry: watchlistEntry, replayed: z.boolean() })
        : watchlistEntry

    if (schema.safeParse(body).success) return { state: "saved" }
  } catch {
    /* An invalid acknowledgement cannot confirm a saved plan. */
  }

  return { state: "failed", reason: "unavailable" }
}

/** Search through the private Axum transport; no provider credential reaches Start or the browser. */
export async function searchWatchlistMedia(
  input: z.infer<typeof mediaSearch>
): Promise<MediaSearchResult> {
  const result = await requestOpenHome({
    path: "/api/watchlist/search",
    method: "GET",
    query: input,
    timeoutMs: 5000,
  })

  if (result.state !== "received" || !result.response.ok)
    return { state: "unavailable" }

  try {
    const parsed = mediaMetadata.array().safeParse(await result.response.json())

    if (parsed.success) return { state: "available", results: parsed.data }
  } catch {
    /* Never expose private provider diagnostics. */
  }

  return { state: "unavailable" }
}
