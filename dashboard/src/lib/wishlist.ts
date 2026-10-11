import { z } from "zod"

/** Household identities and stable destinations returned by Axum. */
export const householdUser = z.object({ id: z.uuid(), name: z.string() })

/** A household-visible named Wishlist. */
export const wishlist = z.object({
  id: z.uuid(),
  owner_id: z.uuid(),
  name: z.string(),
})

/** Minimal generic title item; additional fields belong to the rich-item slice. */
export const wishlistItem = z.object({
  id: z.uuid(),
  list_id: z.uuid(),
  title: z.string(),
  created_at: z.string(),
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

const ownerTarget = z.object({ owner_id: z.uuid() })

const listTarget = ownerTarget.extend({ list_id: z.uuid() })

const itemTarget = listTarget.extend({ item_id: z.uuid() })

/** Protocol shape only; Axum owns text validation and destination policy. */
export const wishlistCommand = z.discriminatedUnion("kind", [
  z.object({ kind: z.literal("create-owner"), name: z.string() }),
  ownerTarget.extend({ kind: z.literal("rename-owner"), name: z.string() }),
  ownerTarget.extend({ kind: z.literal("remove-owner") }),
  ownerTarget.extend({ kind: z.literal("create-list"), name: z.string() }),
  listTarget.extend({ kind: z.literal("rename-list"), name: z.string() }),
  listTarget.extend({ kind: z.literal("remove-list") }),
  listTarget.extend({
    kind: z.literal("add-item"),
    title: z.string(),
    request_id: z.uuid(),
  }),
  itemTarget.extend({ kind: z.literal("edit-item"), title: z.string() }),
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
    "Use a name of 1–200 characters or a wish of 1–500 characters, without control characters.",
  conflict:
    "This name or request is already in use. Refresh and check the destination before trying again.",
  "not-found":
    "This owner, list or wish is no longer available. Refresh to see the current lists.",
}
