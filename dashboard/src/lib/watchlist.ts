import { z } from "zod"
import type { HouseholdUser } from "./wishlist"

/** One movie or whole series planned by an explicit Household User. */
export const watchlistEntry = z.object({
  id: z.uuid(),
  owner_id: z.uuid(),
  kind: z.enum(["movie", "series"]),
  title: z.string(),
  status: z.literal("planned"),
  date_added: z.iso.date(),
})

/** Stored entry returned by authenticated Axum. */
export type WatchlistEntry = z.infer<typeof watchlistEntry>

/** Sanitized transport and API outcomes; private diagnostics never reach the display. */
export type WatchlistFailure =
  | "missing-configuration"
  | "invalid-configuration"
  | "rejected-credential"
  | "unreachable"
  | "unavailable"
  | "invalid"
  | "conflict"
  | "not-found"

/** An incomplete read never represents an empty watchlist. */
export type WatchlistState =
  | { state: "available"; owners: HouseholdUser[]; entries: WatchlistEntry[] }
  | { state: "unavailable"; reason: WatchlistFailure }

/** Explicit destination protocol; Axum owns validation and retry identity. */
export const watchlistCommand = z.discriminatedUnion("action", [
  z.object({
    action: z.literal("add"),
    owner_id: z.uuid(),
    kind: z.enum(["movie", "series"]),
    title: z.string(),
    request_id: z.uuid(),
  }),
  z.object({
    action: z.literal("remove"),
    owner_id: z.uuid(),
    entry_id: z.uuid(),
  }),
])

/** A write always carries its selected owner. */
export type WatchlistCommand = z.infer<typeof watchlistCommand>

/** Confirmation of persistence, separate from uncertain network outcomes. */
export type WatchlistMutation =
  | { state: "saved" }
  | { state: "failed"; reason: WatchlistFailure }

/** Actionable copy without household configuration or upstream error bodies. */
export const watchlistFailureMessage: Record<WatchlistFailure, string> = {
  "missing-configuration":
    "Connect OpenHome in Connection before opening watchlists.",
  "invalid-configuration": "Check the private OpenHome setup in Connection.",
  "rejected-credential": "OpenHome rejected the API key. Check Connection.",
  unreachable:
    "The response was lost. Refresh to check whether the change saved. Retrying the same plan will not duplicate it.",
  unavailable:
    "OpenHome could not confirm the result. Refresh before trying again.",
  invalid:
    "Enter a title of 1–500 characters without control characters, and choose Movie or Whole series.",
  conflict:
    "This request was already accepted or its entry was removed. Refresh to check the list before adding a new plan.",
  "not-found":
    "This member or plan is no longer available. Refresh to see the current watchlists.",
}
