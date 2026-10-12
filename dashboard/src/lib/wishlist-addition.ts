import { z } from "zod"
import { wishlistDetails } from "./wishlist"

const pendingAddition = z.object({
  ownerId: z.uuid(),
  listId: z.uuid(),
  title: z.string(),
  details: wishlistDetails.optional(),
  requestId: z.uuid(),
})

/** A browser-held retry intention, never a second copy of the stored Wishlist. */
export type PendingAddition = z.infer<typeof pendingAddition>

/** Recover a pending wish after refresh or list navigation; ignore unavailable browser storage. */
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

/** Keep the retry ID before dispatch, and remove it only after a confirmed result. */
export function storePendingAddition(
  listId: string,
  pending: PendingAddition | undefined
): void {
  try {
    const key = `openhome.pending-wish.${listId}`

    if (pending === undefined) sessionStorage.removeItem(key)
    else sessionStorage.setItem(key, JSON.stringify(pending))
  } catch {
    /* In-memory retries still work when private browsing disables storage. */
  }
}
