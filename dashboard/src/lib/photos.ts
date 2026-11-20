import { z } from "zod"

export const MAX_ALBUMS = 24

export const PAGE_SIZE = 60

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
    "invalid-date",
  ]),
})

const pageBounds = {
  page: z.number().int().nonnegative(),
  next_page: z.number().int().positive().nullable(),
}

export const albumRead = z.discriminatedUnion("state", [
  z.object({
    state: z.literal("ready"),
    albums: z.array(album).max(MAX_ALBUMS),
  }),
  unavailable,
])

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

export const monthDay = z
  .string()
  .regex(/^(0[1-9]|1[0-2])-(0[1-9]|[12]\d|3[01])$/)
  .refine((value) => {
    const [month = 0, day = 0] = value.split("-").map(Number)

    return new Date(Date.UTC(2000, month - 1, day)).getUTCDate() === day
  })

export const onThisDayRead = z.discriminatedUnion("state", [
  z.object({
    state: z.literal("ready"),
    photos: z.object({
      date: monthDay,
      photo_count: z.number().int().nonnegative(),
      items: z
        .array(
          z.object({
            album: z.uuid(),
            asset: z.uuid(),
            year: z.number().int(),
            years_ago: z.number().int().positive(),
          })
        )
        .max(PAGE_SIZE),
      ...pageBounds,
    }),
  }),
  unavailable,
])

export type AlbumRead = z.infer<typeof albumRead>

export type Album = z.infer<typeof album>

export type WallItem = { album: string; asset: string; year?: number }

export type OnThisDayPhoto = {
  album: string
  asset: string
  year: number
  yearsAgo: number
}

export type OnThisDay = {
  date: string
  items: OnThisDayPhoto[]
  page: number
  next: number | null
  total: number
}

export type OnThisDayResult =
  | { state: "ready"; onThisDay: OnThisDay }
  | z.infer<typeof unavailable>

export type Wall = {
  items: WallItem[]
  page: number
  next: number | null
  total: number
}

export type WallRead =
  | { state: "ready"; wall: Wall }
  | z.infer<typeof unavailable>

export const unavailablePhotos = {
  state: "unavailable",
  reason: "provider-unavailable",
} as const

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

export function mergedWall(read: z.infer<typeof mergedRead>): WallRead {
  if (read.state !== "ready") return read
  const { items, page, next_page, photo_count } = read.photos

  return {
    state: "ready",
    wall: { items, page, next: next_page, total: photo_count },
  }
}

export function onThisDayResult(
  read: z.infer<typeof onThisDayRead>
): OnThisDayResult {
  if (read.state !== "ready") return read
  const { date, items, page, next_page, photo_count } = read.photos

  return {
    state: "ready",
    onThisDay: {
      date,
      items: items.map(({ album, asset, year, years_ago }) => ({
        album,
        asset,
        year,
        yearsAgo: years_ago,
      })),
      page,
      next: next_page,
      total: photo_count,
    },
  }
}

export function onThisDayWall(read: OnThisDayResult): WallRead {
  if (read.state !== "ready") return read
  const { items, page, next, total } = read.onThisDay

  return {
    state: "ready",
    wall: {
      items: items.map(({ album, asset, year }) => ({ album, asset, year })),
      page,
      next,
      total,
    },
  }
}

const dayFormat = new Intl.DateTimeFormat("en-GB", {
  day: "numeric",
  month: "short",
  timeZone: "UTC",
})

export function formatMonthDay(date: string): string {
  return dayFormat.format(new Date(`2000-${date}T00:00:00Z`))
}

export const ALL_PHOTOS = "all"

export const ON_THIS_DAY = "on-this-day"

const view = z.union([z.literal(ALL_PHOTOS), z.literal(ON_THIS_DAY), z.uuid()])

export const photoSearch = z.object({
  album: view.optional().catch(undefined),
  photo: z.uuid().optional().catch(undefined),
})

export type PhotoSearch = z.infer<typeof photoSearch>

export function chooseView(
  albums: Album[],
  requested: string | undefined
): string | undefined {
  if (requested !== undefined) return requested

  return albums.length === 1 ? albums[0]?.id : undefined
}

export const galleryInput = z.object({ album: view.optional() })

export const onThisDayInput = z.object({
  date: monthDay.optional(),
  page: z.number().int().min(0).max(MAX_PAGE).default(0),
})

export const wallInput = z.object({
  album: view,
  page: z.number().int().min(0).max(MAX_PAGE),
})

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
