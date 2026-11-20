import { z } from "zod"
import { trimText } from "./lists"
import { createRequestId } from "./request-id"
import { wishlistDetails, type WishlistDetails } from "./wishlist"

const pendingAddition = z.object({
  ownerId: z.uuid(),
  listId: z.uuid(),
  title: z.string(),
  details: wishlistDetails.optional(),
  requestId: z.uuid(),
})

export type PendingAddition = z.infer<typeof pendingAddition>

export function readPendingAddition(
  listId: string
): PendingAddition | undefined {
  try {
    const serialized = sessionStorage.getItem(`openhome.pending-wish.${listId}`)

    if (serialized === null) return undefined

    const parsed = pendingAddition.safeParse(JSON.parse(serialized))

    return parsed.success && parsed.data.listId === listId
      ? parsed.data
      : undefined
  } catch {
    return undefined
  }
}

export function storePendingAddition(
  listId: string,
  pending: PendingAddition | undefined
): void {
  try {
    const key = `openhome.pending-wish.${listId}`

    if (pending === undefined) sessionStorage.removeItem(key)
    else sessionStorage.setItem(key, JSON.stringify(pending))
  } catch {}
}

function sameDetails(a: WishlistDetails, b: WishlistDetails): boolean {
  return (
    a.title === b.title &&
    (a.url ?? null) === (b.url ?? null) &&
    (a.image_url ?? null) === (b.image_url ?? null) &&
    (a.notes ?? null) === (b.notes ?? null) &&
    (a.price ?? null) === (b.price ?? null) &&
    (a.quantity ?? 1) === (b.quantity ?? 1) &&
    (a.priority ?? "normal") === (b.priority ?? "normal")
  )
}

export function intendAddition(
  known: PendingAddition | undefined,
  ownerId: string,
  listId: string,
  details: WishlistDetails
): PendingAddition {
  return known !== undefined &&
    known.listId === listId &&
    (known.details === undefined
      ? trimText(known.title) === details.title
      : sameDetails(known.details, details))
    ? { ...known, title: details.title, details }
    : {
        ownerId,
        listId,
        title: details.title,
        details,
        requestId: createRequestId(),
      }
}
