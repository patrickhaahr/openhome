import { z } from "zod"

const album = z.object({
  id: z.uuid(),
  name: z.string().min(1).max(200),
  photo_count: z.number().int().nonnegative(),
  cover_id: z.uuid().nullable(),
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

/** Safe selected-album contract; provider details are stripped at the server boundary. */
export const albumRead = z.discriminatedUnion("state", [
  z.object({ state: z.literal("ready"), albums: z.array(album).max(8) }),
  unavailable,
])

/** One bounded page returned by Axum. */
export const photoRead = z.discriminatedUnion("state", [
  z.object({
    state: z.literal("ready"),
    photos: z.object({
      album,
      assets: z.array(z.uuid()).max(60),
      page: z.number().int().nonnegative(),
      next_page: z.number().int().positive().nullable(),
    }),
  }),
  unavailable,
])

/** Selected albums or a sanitized failure. */
export type AlbumRead = z.infer<typeof albumRead>

/** Current photo page or a sanitized failure. */
export type PhotoRead = z.infer<typeof photoRead>

/** A transport failure must never imply an empty library. */
export const unavailablePhotos = {
  state: "unavailable",
  reason: "provider-unavailable",
} as const

/** Validated route and RPC inputs; changing albums starts again at page zero. */
export const photoSearch = z.object({
  album: z.uuid().optional().catch(undefined),
  page: z.number().int().min(0).max(333).optional().catch(undefined),
})

/** A credential-free same-origin resized image URL. */
export function photoUrl(
  albumId: string,
  assetId: string,
  size: "thumbnail" | "preview"
): string {
  return `/photo-media/${encodeURIComponent(albumId)}/${encodeURIComponent(assetId)}/${size}`
}
