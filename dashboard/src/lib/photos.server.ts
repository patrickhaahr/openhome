import "@tanstack/react-start/server-only"
import { z } from "zod"
import { requestOpenHome } from "./openhome.server"
import {
  ALL_PHOTOS,
  ON_THIS_DAY,
  albumRead,
  albumWall,
  mergedRead,
  mergedWall,
  onThisDayRead,
  onThisDayResult,
  onThisDayWall,
  photoRead,
  unavailablePhotos,
  type AlbumRead,
  type OnThisDayResult,
  type WallRead,
} from "./photos"

async function readJson<T>(
  request: Parameters<typeof requestOpenHome>[0],
  schema: z.ZodType<T>
): Promise<T | typeof unavailablePhotos> {
  const result = await requestOpenHome(request)

  if (result.state !== "received") return unavailablePhotos

  try {
    const parsed = schema.safeParse(await result.response.json())

    return parsed.success ? parsed.data : unavailablePhotos
  } catch {
    return unavailablePhotos
  }
}

/** Selected album metadata through the existing server-held OpenHome credential. */
export async function readAlbums(): Promise<AlbumRead> {
  // Each selected album's metadata is read in turn on a cold cache.
  return readJson(
    { path: "/api/photos/albums", method: "GET", timeoutMs: 45000 },
    albumRead
  )
}

/** Photos taken on `date` (`MM-DD`, else today in Copenhagen) in earlier years. */
export async function readOnThisDay(
  date: string | undefined,
  page: number
): Promise<OnThisDayResult> {
  const query =
    date === undefined ? { page: String(page) } : { date, page: String(page) }

  const read = await readJson(
    {
      path: "/api/photos/on-this-day",
      method: "GET",
      timeoutMs: 45000,
      query,
    },
    onThisDayRead
  )

  return onThisDayResult(read)
}

/** One page of one album, of every album merged or of today's photos; Axum checks scope and bounds. */
export async function readWall(album: string, page: number): Promise<WallRead> {
  if (album === ON_THIS_DAY)
    return onThisDayWall(await readOnThisDay(undefined, page))

  if (album === ALL_PHOTOS) {
    const read = await readJson(
      {
        path: "/api/photos/all",
        method: "GET",
        timeoutMs: 45000,
        query: { page: String(page) },
      },
      mergedRead
    )

    return mergedWall(read)
  }

  const read = await readJson(
    {
      path: `/api/photos/albums/${encodeURIComponent(album)}`,
      method: "GET",
      timeoutMs: 7000,
      query: { page: String(page) },
    },
    photoRead
  )

  return albumWall(read)
}

const mediaInput = z.object({
  album: z.uuid(),
  asset: z.uuid(),
  size: z.enum(["thumbnail", "preview"]),
})

/** Forward only bounded resized bytes, never provider headers, redirects or error bodies. */
export async function readPhotoMedia(input: {
  album: string
  asset: string
  size: string
}): Promise<Response> {
  const headers = {
    "Cache-Control": "no-store",
    "X-Content-Type-Options": "nosniff",
  }

  const parsed = mediaInput.safeParse(input)

  if (!parsed.success) return new Response(null, { status: 404, headers })
  const { album, asset, size } = parsed.data

  const result = await requestOpenHome({
    path: `/api/photos/albums/${album}/assets/${asset}/${size}`,
    method: "GET",
    timeoutMs: 12000,
  })

  if (result.state !== "received")
    return new Response(null, { status: 503, headers })
  const response = result.response
  const contentType = response.headers.get("content-type")

  if (
    !response.ok ||
    !["image/jpeg", "image/webp", "image/png"].includes(contentType ?? "")
  ) {
    await response.body?.cancel()

    return new Response(null, {
      status: response.status === 404 ? 404 : 503,
      headers,
    })
  }

  // Defend against a misconfigured/older Axum too; hold a single bounded body.
  const reader = response.body?.getReader()

  if (reader === undefined) return new Response(null, { status: 503, headers })
  const limit = size === "thumbnail" ? 2 * 1024 * 1024 : 12 * 1024 * 1024
  const chunks: Uint8Array[] = []
  let length = 0

  try {
    for (;;) {
      const chunk = await reader.read()

      if (chunk.done) break
      length += chunk.value.byteLength

      if (length > limit) {
        await reader.cancel()

        return new Response(null, { status: 503, headers })
      }

      chunks.push(chunk.value)
    }

    const body = new Uint8Array(length)
    let offset = 0

    for (const chunk of chunks) {
      body.set(chunk, offset)
      offset += chunk.byteLength
    }

    return new Response(body, {
      headers: { ...headers, "Content-Type": contentType ?? "image/jpeg" },
    })
  } catch {
    return new Response(null, { status: 503, headers })
  }
}
