import { z } from "zod"
import type { HouseholdUser } from "./wishlist"

/** One movie or whole series planned by an explicit Household User. */
export const watchlistEntry = z.object({
  id: z.uuid(),
  owner_id: z.uuid(),
  kind: z.enum(["movie", "series"]),
  title: z.string(),
  status: z.enum(["planned", "watching", "completed"]),
  date_added: z.iso.date(),
  completion_date: z.iso.date().nullable(),
})

/** URL filters keep the household view recoverable after navigation and refresh. */
export const watchlistFilters = z.object({
  owner_id: z.uuid().optional().catch(undefined),
  status: watchlistEntry.shape.status.optional().catch(undefined),
})

export type WatchlistFilters = z.infer<typeof watchlistFilters>

export const watchStatusLabels: Record<WatchlistEntry["status"], string> = {
  planned: "Planned",
  watching: "Watching",
  completed: "Completed",
}

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
    action: z.literal("edit"),
    owner_id: z.uuid(),
    entry_id: z.uuid(),
    status: watchlistEntry.shape.status.optional(),
    date_added: z.iso.date().optional(),
    completion_date: z.iso.date().nullable().optional(),
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
    "Check the title, media type and dates. Completed entries need a real completion date on or after the added date.",
  conflict:
    "This request conflicts with a saved change or its entry was removed. Refresh to check the list before retrying.",
  "not-found":
    "This member or plan is no longer available. Refresh to see the current watchlists.",
}
