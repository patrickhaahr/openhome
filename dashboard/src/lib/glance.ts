import { getGallery } from "./photos.functions"
import { loadActiveRoute } from "./route-loop.functions"
import { titleHue } from "./watchlist"
import { loadWatchlists } from "./watchlist.functions"
import { loadWishlists } from "./wishlist.functions"
import { loadWorkouts } from "./workout.functions"

/** Household wishes at a glance. */
export type WishlistGlance = {
  readonly wishes: number
  readonly lists: number
  readonly people: number
}

/** What is being watched or planned next, the next few as posters. */
export type WatchlistGlance = {
  readonly planned: number
  readonly watching: number
  readonly next: string | null
  readonly posters: readonly {
    readonly id: string
    readonly title: string
    readonly hue: number
    readonly poster: string | null
  }[]
}

/** The selected loop as a drawable outline, or nothing selected. */
export type TravelGlance = {
  /** An SVG path in a 0..100 box; null when no route is selected. */
  readonly path: string | null
  readonly meters: number | null
}

/** Logged sessions in each of the last eight Copenhagen weeks, oldest first. */
export type WorkoutGlance = {
  readonly last: string | null
  readonly weeks: readonly number[]
}

/** Album covers for the Home row. */
export type PhotosGlance = {
  readonly photos: number
  readonly covers: readonly { readonly album: string; readonly asset: string }[]
}

/**
 * One line of live value per Home directory row, each deferred and independent, so no read
 * delays the clock or the controls. `null` means the value could not be read, never "empty".
 */
export type Glance = {
  readonly wishlist: Promise<WishlistGlance | null>
  readonly watchlist: Promise<WatchlistGlance | null>
  readonly workout: Promise<WorkoutGlance | null>
  readonly photos: Promise<PhotosGlance | null>
  readonly travel: Promise<TravelGlance | null>
}

const day = 86_400_000

export function loadGlance(now: Date): Glance {
  return {
    wishlist: loadWishlists()
      .then((read) =>
        read.state === "available"
          ? {
              wishes: read.items.length,
              lists: read.lists.length,
              people: read.owners.length,
            }
          : null
      )
      .catch(() => null),
    watchlist: loadWatchlists()
      .then((read) => {
        if (read.state !== "available") return null

        const open = read.entries.filter(
          (entry) => entry.status !== "completed"
        )

        const watching = open.filter((entry) => entry.status === "watching")

        const upcoming = [
          ...watching,
          ...open.filter((entry) => entry.status !== "watching"),
        ]

        return {
          planned: open.length - watching.length,
          watching: watching.length,
          next: upcoming[0]?.title ?? null,
          posters: upcoming.slice(0, 3).map((entry) => ({
            id: entry.id,
            title: entry.title,
            hue: titleHue(entry.title),
            poster: entry.poster_path,
          })),
        }
      })
      .catch(() => null),
    workout: loadWorkouts({})
      .then((read) => {
        if (read.state !== "available") return null

        const weeks = Array.from({ length: 8 }, () => 0)

        for (const workout of read.history) {
          const age = Math.floor(
            (now.getTime() - Date.parse(`${workout.date}T12:00:00Z`)) / day
          )

          const week = 7 - Math.floor(age / 7)

          if (week >= 0 && week < 8) weeks[week] = (weeks[week] ?? 0) + 1
        }

        return { last: read.history[0]?.date ?? null, weeks }
      })
      .catch(() => null),
    photos: getGallery({ data: {} })
      .then(({ albums }) =>
        albums.state === "ready"
          ? {
              photos: albums.albums.reduce(
                (sum, album) => sum + album.photo_count,
                0
              ),
              covers: albums.albums.flatMap((album) =>
                album.cover_id === null || album.missing
                  ? []
                  : [{ album: album.id, asset: album.cover_id }]
              ),
            }
          : null
      )
      .catch(() => null),
    travel: loadActiveRoute()
      .then((read) => {
        if (read.state === "unavailable") return null

        if (read.state === "none") return { path: null, meters: null }

        return {
          path: outline(read.route.geometry.coordinates),
          meters: read.route.distance_meters,
        }
      })
      .catch(() => null),
  }
}

/**
 * A route's outline fitted into a 0..100 box, longitude scaled by latitude so the loop keeps
 * its outline. Only the outline leaves the server's map; no coordinate is drawn.
 */
function outline(coordinates: readonly (readonly [number, number])[]): string {
  const middle =
    coordinates.reduce((sum, [, latitude]) => sum + latitude, 0) /
    coordinates.length

  const scale = Math.cos((middle * Math.PI) / 180)
  const xs = coordinates.map(([longitude]) => longitude * scale)
  const ys = coordinates.map(([, latitude]) => -latitude)
  const left = Math.min(...xs)
  const top = Math.min(...ys)
  const span = Math.max(Math.max(...xs) - left, Math.max(...ys) - top) || 1
  const offsetX = (100 - ((Math.max(...xs) - left) / span) * 100) / 2
  const offsetY = (100 - ((Math.max(...ys) - top) / span) * 100) / 2

  return coordinates
    .map(
      (_, index) =>
        `${index === 0 ? "M" : "L"}${(offsetX + (((xs[index] ?? 0) - left) / span) * 100).toFixed(1)},${(offsetY + (((ys[index] ?? 0) - top) / span) * 100).toFixed(1)}`
    )
    .join(" ")
}
