import "@tanstack/react-start/server-only"
import { z } from "zod"
import { requestOpenHome } from "./openhome.server"
import {
  householdUser,
  wishlist,
  wishlistItem,
  type WishlistCommand,
  type WishlistFailure,
  type WishlistMutation,
  type WishlistState,
} from "./wishlist"

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
