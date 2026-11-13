import { z } from "zod"
import type { ConnectionState } from "./connection"
import { personSearch } from "./lists"
import type { HouseholdUser } from "./wishlist"

/** The Binder pages: whose Binder (the first Household User when absent) and which spread, from 1. */
export const binderSearch = personSearch.extend({
  page: z.number().int().min(1).optional().catch(undefined),
})

export type BinderSearch = z.infer<typeof binderSearch>

/** Axum's Card id is the catalogue's opaque image identifier, safe in a URL path. */
export const cardId = z.string().regex(/^[A-Za-z0-9][A-Za-z0-9_-]{0,63}$/)

/** One Card in one language in one owner's Binder; only what a pocket draws. */
export const binderEntry = z.object({
  id: z.uuid(),
  owner_id: z.uuid(),
  language: z.enum(["en", "ja"]),
  quantity: z.number().int().positive(),
  card: z.object({ id: cardId, card_code: z.string(), name: z.string() }),
})

export type BinderEntry = z.infer<typeof binderEntry>

export type BinderFailure = Exclude<ConnectionState, "connected">

/** An incomplete read never represents an empty Binder. Entries arrive in Card Code order. */
export type BinderState =
  | {
      readonly state: "available"
      readonly owners: HouseholdUser[]
      readonly entries: BinderEntry[]
    }
  | { readonly state: "unavailable"; readonly reason: BinderFailure }

export const binderFailureMessage: Record<BinderFailure, string> = {
  "missing-configuration":
    "Connect OpenHome in Connection before opening Binder.",
  "invalid-configuration": "Check the private OpenHome setup in Connection.",
  "rejected-credential": "OpenHome rejected the API key. Check Connection.",
  unreachable: "OpenHome did not answer. Check again in a moment.",
  unavailable: "OpenHome could not read the Binders. Check again in a moment.",
}

/** A pocket page is 3×3. */
export const pocketsPerPage = 9

/** A spread is two facing pages: the left holds the first nine pockets, the right the next nine. */
export const pocketsPerSpread = 2 * pocketsPerPage

/** One spread of pockets, with the zero-based spread index clamped into range. */
export type Spread<T> = {
  left: T[]
  right: T[]
  index: number
  count: number
}

export function paginate<T>(entries: readonly T[], index: number): Spread<T> {
  const count = Math.max(1, Math.ceil(entries.length / pocketsPerSpread))
  const at = Math.min(Math.max(index, 0), count - 1)
  const start = at * pocketsPerSpread
  const middle = start + pocketsPerPage

  return {
    left: entries.slice(start, middle),
    right: entries.slice(middle, start + pocketsPerSpread),
    index: at,
    count,
  }
}

/** What the page shows, decided from one read and the URL. */
export type BinderView =
  | { readonly view: "unavailable"; readonly reason: BinderFailure }
  | { readonly view: "no-household" }
  | {
      readonly view: "empty"
      readonly owners: HouseholdUser[]
      owner: HouseholdUser
    }
  | {
      readonly view: "spread"
      readonly owners: HouseholdUser[]
      readonly owner: HouseholdUser
      readonly left: BinderEntry[]
      readonly right: BinderEntry[]
      readonly index: number
      readonly count: number
    }

export function viewOf(state: BinderState, search: BinderSearch): BinderView {
  if (state.state === "unavailable")
    return { view: "unavailable", reason: state.reason }

  const { owners } = state
  const first = owners[0]

  if (first === undefined) return { view: "no-household" }

  const owner = owners.find((user) => user.id === search.person) ?? first

  const mine = state.entries.filter((entry) => entry.owner_id === owner.id)

  if (mine.length === 0) return { view: "empty", owners, owner }

  return {
    view: "spread",
    owners,
    owner,
    ...paginate(mine, (search.page ?? 1) - 1),
  }
}

/** The Dashboard Start route that relays Axum's stored image; the browser never sees the catalogue. */
export function cardImageUrl(id: string): string {
  return `/binder-image/${id}`
}
