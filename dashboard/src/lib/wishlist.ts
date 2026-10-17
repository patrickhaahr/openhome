import { z } from "zod"

/** Household identities and stable destinations returned by Axum. */
export const householdUser = z.object({ id: z.uuid(), name: z.string() })

/** A household-visible named Wishlist. */
export const wishlist = z.object({
  id: z.uuid(),
  owner_id: z.uuid(),
  name: z.string(),
})

/** Rich desired item, without purchase or reservation state. */
export const wishlistItem = z.object({
  id: z.uuid(),
  list_id: z.uuid(),
  title: z.string(),
  created_at: z.string(),
  url: z.string().nullable(),
  image_url: z.string().nullable(),
  notes: z.string().nullable(),
  price: z.string().nullable(),
  quantity: z.number().int(),
  priority: z.enum(["low", "normal", "high"]),
})

/** One configured person, without authentication or active-user state. */
export type HouseholdUser = z.infer<typeof householdUser>

/** A named collection owned by a configured person. */
export type Wishlist = z.infer<typeof wishlist>

/** An existing generic desired thing. */
export type WishlistItem = z.infer<typeof wishlistItem>

/** Sanitized failure classes; upstream diagnostic text never reaches the browser. */
export type WishlistFailure =
  | "missing-configuration"
  | "invalid-configuration"
  | "rejected-credential"
  | "unreachable"
  | "unavailable"
  | "invalid"
  | "conflict"
  | "not-found"

/** Complete shared read, or an unavailable outcome that must never look like an empty household. */
export type WishlistState =
  | {
      readonly state: "available"
      readonly owners: HouseholdUser[]
      readonly lists: Wishlist[]
      readonly items: WishlistItem[]
    }
  | { readonly state: "unavailable"; readonly reason: WishlistFailure }

/** Mutation receipts describe persistence separately from an uncertain transport outcome. */
export type WishlistMutation =
  | { readonly state: "saved" }
  | { readonly state: "failed"; readonly reason: WishlistFailure }

/** Item write protocol; Axum owns field limits and HTTP(S) URL validation. */
export const wishlistDetails = z.object({
  title: z.string(),
  url: z.string().nullable().optional(),
  image_url: z.string().nullable().optional(),
  notes: z.string().nullable().optional(),
  price: z.string().nullable().optional(),
  quantity: z.number().optional(),
  priority: z.string().optional(),
})

/** Editable item fields carried unchanged to the API. */
export type WishlistDetails = z.infer<typeof wishlistDetails>

const ownerTarget = z.object({ owner_id: z.uuid() })

const listTarget = ownerTarget.extend({ list_id: z.uuid() })

/** Explicit destination for a non-persistent metadata preview. */
export const wishlistMetadataRequest = listTarget.extend({ url: z.string() })

/** Editable suggestions, never a persistence receipt. */
export const wishlistMetadata = z.object({
  status: z.enum(["ready", "partial", "unavailable"]),
  url: z.string(),
  title: z.string().nullable(),
  image_url: z.string().nullable(),
  price: z.string().nullable(),
})

/** A successful read can still report unavailable extraction and allow manual entry. */
export type WishlistMetadataResult =
  | { state: "preview"; data: z.infer<typeof wishlistMetadata> }
  | { state: "failed"; reason: WishlistFailure }

/** Shared preview operation for the selected owner/list. */
export type WishlistMetadataLoader = (
  request: z.infer<typeof wishlistMetadataRequest>
) => Promise<WishlistMetadataResult>

const itemTarget = listTarget.extend({ item_id: z.uuid() })

/** Protocol shape only; Axum owns text validation and destination policy. */
export const wishlistCommand = z.discriminatedUnion("kind", [
  z.object({ kind: z.literal("create-owner"), name: z.string() }),
  ownerTarget.extend({ kind: z.literal("rename-owner"), name: z.string() }),
  ownerTarget.extend({ kind: z.literal("remove-owner") }),
  ownerTarget.extend({ kind: z.literal("create-list"), name: z.string() }),
  listTarget.extend({ kind: z.literal("rename-list"), name: z.string() }),
  listTarget.extend({ kind: z.literal("remove-list") }),
  listTarget.extend(wishlistDetails.shape).extend({
    kind: z.literal("add-item"),
    request_id: z.uuid(),
  }),
  itemTarget
    .extend(wishlistDetails.partial().shape)
    .extend({ kind: z.literal("edit-item") }),
  itemTarget.extend({ kind: z.literal("remove-item") }),
])

/** Every write carries the intended stable destination. */
export type WishlistCommand = z.infer<typeof wishlistCommand>

/** Actionable static copy, safe for public failure evidence. */
export const wishlistFailureMessage: Record<WishlistFailure, string> = {
  "missing-configuration":
    "Connect OpenHome in Connection before opening wishlists.",
  "invalid-configuration": "Check the private OpenHome setup in Connection.",
  "rejected-credential": "OpenHome rejected the API key. Check Connection.",
  unreachable:
    "The response was lost. Refresh to check whether the change saved before trying again. Retrying the same wish will not duplicate it.",
  unavailable:
    "OpenHome could not confirm the result. Refresh before trying again.",
  invalid:
    "Check the fields: title 1–500 characters, notes up to 5000, price up to 100, quantity 1–999, and priority low, normal or high. Links must use HTTP(S) without credentials. Names allow 1–200 characters. Only notes allow line breaks and tabs.",
  conflict:
    "This name or request is already in use. Refresh and check the destination before trying again.",
  "not-found":
    "This owner, list or wish is no longer available. Refresh to see the current lists.",
}

/**
 * The wish's picture, relayed through the dashboard so the API key stays on the server and the
 * shop is never contacted by the browser. The version changes when the stored address does.
 */
export function wishImageUrl(
  ownerId: string,
  item: WishlistItem
): string | null {
  if (item.image_url === null) return null
  let hash = 0x811c9dc5

  for (const character of item.image_url) {
    hash ^= character.codePointAt(0) ?? 0
    hash = Math.imul(hash, 0x01000193)
  }

  const version = (hash >>> 0).toString(36)

  return `/wish-image/${ownerId}/${item.list_id}/${item.id}?v=${version}`
}
