import { createServerFn } from "@tanstack/react-start"
import { setResponseHeader } from "@tanstack/react-start/server"
import { mediaSearch, watchlistCommand, type WatchlistState } from "./watchlist"

/** Household-visible read through authenticated Axum, with credentials held by Start. */
export const getWatchlists = createServerFn({ method: "GET" }).handler(
  async () => {
    setResponseHeader("Cache-Control", "no-store")
    const { readWatchlists } = await import("./watchlist.server")

    return readWatchlists()
  }
)

/** Explicit-owner writes use the same Axum rules and receipts as MCP. */
export const mutateWatchlist = createServerFn({ method: "POST" })
  .validator(watchlistCommand)
  .handler(async ({ data }) => {
    setResponseHeader("Cache-Control", "no-store")
    const { writeWatchlist } = await import("./watchlist.server")

    return writeWatchlist(data)
  })

/** A dropped RPC keeps recovery reachable within the focused view. */
export async function loadWatchlists(): Promise<WatchlistState> {
  try {
    return await getWatchlists()
  } catch {
    return { state: "unavailable", reason: "unreachable" }
  }
}

/** Deliberate provider lookup behind Start's server boundary. */
export const searchWatchlist = createServerFn({ method: "GET" })
  .validator(mediaSearch)
  .handler(async ({ data }) => {
    setResponseHeader("Cache-Control", "no-store")
    const { searchWatchlistMedia } = await import("./watchlist.server")

    return searchWatchlistMedia(data)
  })
