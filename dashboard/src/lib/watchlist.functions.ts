import { createServerFn } from "@tanstack/react-start"
import { setResponseHeader } from "@tanstack/react-start/server"
import { mediaSearch, watchlistCommand, type WatchlistState } from "./watchlist"

export const getWatchlists = createServerFn({ method: "GET" }).handler(
  async () => {
    setResponseHeader("Cache-Control", "no-store")
    const { readWatchlists } = await import("./watchlist.server")

    return readWatchlists()
  }
)

export const mutateWatchlist = createServerFn({ method: "POST" })
  .validator(watchlistCommand)
  .handler(async ({ data }) => {
    setResponseHeader("Cache-Control", "no-store")
    const { writeWatchlist } = await import("./watchlist.server")

    return writeWatchlist(data)
  })

export async function loadWatchlists(): Promise<WatchlistState> {
  try {
    return await getWatchlists()
  } catch {
    return { state: "unavailable", reason: "unreachable" }
  }
}

export const searchWatchlist = createServerFn({ method: "GET" })
  .validator(mediaSearch)
  .handler(async ({ data }) => {
    setResponseHeader("Cache-Control", "no-store")
    const { searchWatchlistMedia } = await import("./watchlist.server")

    return searchWatchlistMedia(data)
  })
