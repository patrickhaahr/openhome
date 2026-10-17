import { z } from "zod"

/** Axum's bounds: albums in `IMMICH_ALBUM_IDS`, photo identities per page. */
export const MAX_ALBUMS = 24

export const PAGE_SIZE = 60

// 24 albums of at most 20,000 photos each, 60 to a page.
const MAX_PAGE = 8000

const month = z.string().regex(/^\d{4}-(0[1-9]|1[0-2])$/)

const album = z.object({
  id: z.uuid(),
  name: z.string().min(1).max(200),
  photo_count: z.number().int().nonnegative(),
  cover_id: z.uuid().nullable(),
  first_month: month.nullable(),
  last_month: month.nullable(),
  missing: z.boolean(),
})

const unavailable = z.object({
  state: z.literal("unavailable"),
  reason: z.enum([
    "missing-configuration",
    "provider-unavailable",
    "not-found",
    "invalid-page",
  ]),
})

const pageBounds = {
  page: z.number().int().nonnegative(),
  next_page: z.number().int().positive().nullable(),
}

/** Safe selected-album contract; provider details are stripped at the server boundary. */
export const albumRead = z.discriminatedUnion("state", [
  z.object({
    state: z.literal("ready"),
    albums: z.array(album).max(MAX_ALBUMS),
  }),
  unavailable,
])

/** One bounded page of one album, as Axum returns it. */
export const photoRead = z.discriminatedUnion("state", [
  z.object({
    state: z.literal("ready"),
    photos: z.object({
      album,
      assets: z.array(z.uuid()).max(PAGE_SIZE),
      ...pageBounds,
    }),
  }),
  unavailable,
])

/** One bounded page across every selected album, newest first. */
export const mergedRead = z.discriminatedUnion("state", [
  z.object({
    state: z.literal("ready"),
    photos: z.object({
      photo_count: z.number().int().nonnegative(),
      items: z
        .array(z.object({ album: z.uuid(), asset: z.uuid() }))
        .max(PAGE_SIZE),
      ...pageBounds,
    }),
  }),
  unavailable,
])

/** Selected albums or a sanitized failure. */
export type AlbumRead = z.infer<typeof albumRead>

export type Album = z.infer<typeof album>

/** A photo and the selected album that grants access to it. */
export type WallItem = { album: string; asset: string }

/** One loaded page of the photo wall, from one album or from all of them. */
export type Wall = {
  items: WallItem[]
  page: number
  next: number | null
  total: number
}

export type WallRead =
  | { state: "ready"; wall: Wall }
  | z.infer<typeof unavailable>

/** A transport failure must never imply an empty library. */
export const unavailablePhotos = {
  state: "unavailable",
  reason: "provider-unavailable",
} as const

/** One album's page as the wall reads it. */
export function albumWall(read: z.infer<typeof photoRead>): WallRead {
  if (read.state !== "ready") return read
  const { album: source, assets, page, next_page } = read.photos

  return {
    state: "ready",
    wall: {
      items: assets.map((asset) => ({ album: source.id, asset })),
      page,
      next: next_page,
      total: source.photo_count,
    },
  }
}

/** The merged page as the wall reads it. */
export function mergedWall(read: z.infer<typeof mergedRead>): WallRead {
  if (read.state !== "ready") return read
  const { items, page, next_page, photo_count } = read.photos

  return {
    state: "ready",
    wall: { items, page, next: next_page, total: photo_count },
  }
}

/** Every selected album at once. */
export const ALL_PHOTOS = "all"

const view = z.union([z.literal(ALL_PHOTOS), z.uuid()])

/**
 * The open album (or all photos) and the open photo live in the URL. Without an album the page
 * shows the albums, unless only one is configured.
 */
export const photoSearch = z.object({
  album: view.optional().catch(undefined),
  photo: z.uuid().optional().catch(undefined),
})

export type PhotoSearch = z.infer<typeof photoSearch>

/** The view to open: the requested one, else the only album when just one is configured. */
export function chooseView(
  albums: Album[],
  requested: string | undefined
): string | undefined {
  if (requested !== undefined) return requested

  return albums.length === 1 ? albums[0]?.id : undefined
}

/** The first page of the chosen view; the server falls back to the only album. */
export const galleryInput = z.object({ album: view.optional() })

/** A later page, loaded as the wall scrolls. */
export const wallInput = z.object({
  album: view,
  page: z.number().int().min(0).max(MAX_PAGE),
})

/** A credential-free same-origin resized image URL. */
export function photoUrl(
  albumId: string,
  assetId: string,
  size: "thumbnail" | "preview"
): string {
  return `/photo-media/${encodeURIComponent(albumId)}/${encodeURIComponent(assetId)}/${size}`
}

const monthFormat = new Intl.DateTimeFormat("en-GB", {
  month: "short",
  year: "numeric",
  timeZone: "UTC",
})

const monthOnly = new Intl.DateTimeFormat("en-GB", {
  month: "short",
  timeZone: "UTC",
})

/** The months an album covers: "Jun 2025", "Jun – Aug 2025" or "Dec 2024 – Feb 2025". */
export function monthSpan(
  first: string | null,
  last: string | null
): string | null {
  if (first === null || last === null) return null
  const start = new Date(`${first}-01T00:00:00Z`)
  const end = new Date(`${last}-01T00:00:00Z`)

  if (first === last) return monthFormat.format(start)

  if (first.slice(0, 4) === last.slice(0, 4))
    return `${monthOnly.format(start)} – ${monthFormat.format(end)}`

  return `${monthFormat.format(start)} – ${monthFormat.format(end)}`
}

/** The span of every available album together. */
export function librarySpan(albums: Album[]): string | null {
  const firsts = albums.flatMap((item) =>
    item.first_month === null ? [] : [item.first_month]
  )

  const lasts = albums.flatMap((item) =>
    item.last_month === null ? [] : [item.last_month]
  )

  if (firsts.length === 0 || lasts.length === 0) return null

  return monthSpan(
    firsts.reduce((a, b) => (b < a ? b : a)),
    lasts.reduce((a, b) => (b > a ? b : a))
  )
}
