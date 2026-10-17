import "@tanstack/react-start/server-only"
import { z } from "zod"
import { requestOpenHome } from "./openhome.server"
import {
  householdUser,
  wishlist,
  wishlistItem,
  wishlistMetadata,
  wishlistMetadataRequest,
  type WishlistMetadataResult,
  type WishlistCommand,
  type WishlistFailure,
  type WishlistMutation,
  type WishlistState,
} from "./wishlist"

/** Fetch suggestions from Axum; credentials and fetching policy stay on the server. */
export async function previewWishlistUrl(
  request: z.infer<typeof wishlistMetadataRequest>
): Promise<WishlistMetadataResult> {
  const result = await requestOpenHome({
    path: "/api/wishlist/url-metadata",
    method: "POST",
    json: request,
    timeoutMs: 10000,
  })

  if (result.state !== "received")
    return { state: "failed", reason: result.state }

  if (!result.response.ok)
    return { state: "failed", reason: reason(result.response.status) }

  try {
    const parsed = wishlistMetadata.safeParse(await result.response.json())

    if (parsed.success) return { state: "preview", data: parsed.data }
  } catch {
    /* Only sanitized outcomes reach the browser. */
  }

  return { state: "failed", reason: "unavailable" }
}

function reason(status: number): WishlistFailure {
  if (status === 400 || status === 422) return "invalid"

  if (status === 404) return "not-found"

  if (status === 409) return "conflict"

  return "unavailable"
}

async function read<T>(
  path: `/api/${string}`,
  schema: z.ZodType<T>
): Promise<
  { state: "read"; data: T } | { state: "unavailable"; reason: WishlistFailure }
> {
  const result = await requestOpenHome({ path, method: "GET", timeoutMs: 5000 })

  if (result.state !== "received")
    return { state: "unavailable", reason: result.state }

  if (!result.response.ok)
    return { state: "unavailable", reason: reason(result.response.status) }

  try {
    const parsed = schema.safeParse(await result.response.json())

    return parsed.success
      ? { state: "read", data: parsed.data }
      : { state: "unavailable", reason: "unavailable" }
  } catch {
    return { state: "unavailable", reason: "unavailable" }
  }
}

/** Read all owners and lists from authenticated Axum; incomplete reads never become empty lists. */
export async function readWishlists(): Promise<WishlistState> {
  const [owners, lists] = await Promise.all([
    read("/api/household-users", householdUser.array()),
    read("/api/wishlists", wishlist.array()),
  ])

  if (owners.state !== "read") return owners

  if (lists.state !== "read") return lists

  const results = await Promise.all(
    lists.data.map((list) =>
      read(`/api/wishlists/${list.id}/items`, wishlistItem.array())
    )
  )

  const items = []

  for (const result of results) {
    if (result.state !== "read") return result
    items.push(...result.data)
  }

  return { state: "available", owners: owners.data, lists: lists.data, items }
}

type WishlistOperation = {
  path: `/api/${string}`
  method: "POST" | "PATCH" | "DELETE"
  json?: Record<string, string | number | null | undefined>
  schema?: z.ZodType
}

function details(
  command: Extract<WishlistCommand, { kind: "add-item" | "edit-item" }>
) {
  const { title, url, image_url, notes, price, quantity, priority } = command

  return { title, url, image_url, notes, price, quantity, priority }
}

function operation(command: WishlistCommand): WishlistOperation {
  switch (command.kind) {
    case "create-owner":
      return {
        path: "/api/household-users",
        method: "POST",
        json: { name: command.name },
        schema: householdUser,
      }
    case "rename-owner":
      return {
        path: `/api/household-users/${command.owner_id}`,
        method: "PATCH",
        json: { name: command.name },
        schema: householdUser,
      }
    case "remove-owner":
      return {
        path: `/api/household-users/${command.owner_id}`,
        method: "DELETE",
      }
    case "create-list":
      return {
        path: "/api/wishlists",
        method: "POST",
        json: { owner_id: command.owner_id, name: command.name },
        schema: wishlist,
      }
    case "rename-list":
      return {
        path: `/api/household-users/${command.owner_id}/wishlists/${command.list_id}`,
        method: "PATCH",
        json: { name: command.name },
        schema: wishlist,
      }
    case "remove-list":
      return {
        path: `/api/household-users/${command.owner_id}/wishlists/${command.list_id}`,
        method: "DELETE",
      }
    case "add-item":
      return {
        path: "/api/wishlist/items",
        method: "POST",
        json: {
          owner_id: command.owner_id,
          list_id: command.list_id,
          ...details(command),
          request_id: command.request_id,
        },
        schema: z.object({ item: wishlistItem, replayed: z.boolean() }),
      }
    case "edit-item":
      return {
        path: `/api/household-users/${command.owner_id}/wishlists/${command.list_id}/items/${command.item_id}`,
        method: "PATCH",
        json: details(command),
        schema: wishlistItem,
      }
    case "remove-item":
      return {
        path: `/api/household-users/${command.owner_id}/wishlists/${command.list_id}/items/${command.item_id}`,
        method: "DELETE",
      }
  }
}

/** Forward one typed write; only a parsed API receipt confirms persistence. */
export async function writeWishlist(
  command: WishlistCommand
): Promise<WishlistMutation> {
  const { schema, ...request } = operation(command)
  const result = await requestOpenHome({ ...request, timeoutMs: 5000 })

  if (result.state !== "received")
    return { state: "failed", reason: result.state }

  if (!result.response.ok)
    return { state: "failed", reason: reason(result.response.status) }

  if (request.method === "DELETE")
    return result.response.status === 204
      ? { state: "saved" }
      : { state: "failed", reason: "unavailable" }

  try {
    if (
      schema !== undefined &&
      schema.safeParse(await result.response.json()).success
    )
      return { state: "saved" }
  } catch {
    /* Never expose upstream error bodies or private URLs. */
  }

  return { state: "failed", reason: "unavailable" }
}

const wishImageInput = z.object({
  owner: z.uuid(),
  list: z.uuid(),
  item: z.uuid(),
})

// SVG only ever comes from the synthetic preview; Axum relays raster pictures alone.
const wishImageTypes = [
  "image/jpeg",
  "image/png",
  "image/webp",
  "image/gif",
  "image/avif",
  "image/svg+xml",
]

const wishImageLimit = 5 * 1024 * 1024

/** Relay one stored wish picture as bounded bytes, never provider headers or error bodies. */
export async function readWishImage(input: {
  owner: string
  list: string
  item: string
}): Promise<Response> {
  const headers = {
    "Cache-Control": "private, max-age=3600",
    "X-Content-Type-Options": "nosniff",
    "Content-Security-Policy":
      "default-src 'none'; style-src 'unsafe-inline'; sandbox",
  }

  const failed = (status: number): Response =>
    new Response(null, {
      status,
      headers: { ...headers, "Cache-Control": "no-store" },
    })

  const parsed = wishImageInput.safeParse(input)

  if (!parsed.success) return failed(404)
  const { owner, list, item } = parsed.data

  const result = await requestOpenHome({
    path: `/api/household-users/${owner}/wishlists/${list}/items/${item}/image`,
    method: "GET",
    timeoutMs: 12000,
  })

  if (result.state !== "received") return failed(503)
  const response = result.response
  const contentType = response.headers.get("content-type") ?? ""

  if (!response.ok || !wishImageTypes.includes(contentType)) {
    await response.body?.cancel()

    return failed(response.status === 404 ? 404 : 503)
  }

  const reader = response.body?.getReader()

  if (reader === undefined) return failed(503)
  const chunks: Uint8Array[] = []
  let length = 0

  try {
    for (;;) {
      const chunk = await reader.read()

      if (chunk.done) break
      length += chunk.value.byteLength

      if (length > wishImageLimit) {
        await reader.cancel()

        return failed(503)
      }

      chunks.push(chunk.value)
    }
  } catch {
    return failed(503)
  }

  const body = new Uint8Array(length)
  let offset = 0

  for (const chunk of chunks) {
    body.set(chunk, offset)
    offset += chunk.byteLength
  }

  return new Response(body, {
    headers: { ...headers, "Content-Type": contentType },
  })
}
