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

/** Native secure randomness works on the private display's HTTP origin as well as HTTPS. */
export function createWishRequestId(): string {
  const bytes = crypto.getRandomValues(new Uint8Array(16))

  bytes[6] = ((bytes[6] ?? 0) & 15) | 64
  bytes[8] = ((bytes[8] ?? 0) & 63) | 128

  const hex = Array.from(bytes, (byte) =>
    byte.toString(16).padStart(2, "0")
  ).join("")

  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`
}
