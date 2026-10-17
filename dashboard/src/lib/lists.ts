import { z } from "zod"

const id = z.uuid().optional().catch(undefined)

/** The chosen Household User, held in the URL; absent means everyone. */
export const personSearch = z.object({ person: id })

/** The gift board also narrows to one named Wishlist of the chosen person. */
export const wishSearch = personSearch.extend({ list: id })

/** The Lists page: which list kind is showing (wishes when absent), with one shared person filter. */
export const listsSearch = wishSearch.extend({
  tab: z.enum(["wish", "watch"]).optional().catch(undefined),
})

export type PersonSearch = z.infer<typeof personSearch>

export type WishSearch = z.infer<typeof wishSearch>

const dayFormat = new Intl.DateTimeFormat("en-GB", {
  day: "numeric",
  month: "short",
  year: "numeric",
  timeZone: "UTC",
})

/** A calendar date (YYYY-MM-DD) as "6 Oct 2026"; the date has no time zone of its own. */
export function formatDay(date: string): string {
  return dayFormat.format(new Date(`${date}T00:00:00Z`))
}

/** A person's mark on their key and on shared plates. */
export function initial(name: string): string {
  return Array.from(name.trim())[0]?.toLocaleUpperCase("en-GB") ?? "?"
}

/** Match Axum's Rust str::trim(): Unicode White_Space, unlike JavaScript's trim(). */
export function trimText(value: string): string {
  return value.replace(/^\p{White_Space}+|\p{White_Space}+$/gu, "")
}

/** Persistence-confirmed writes refresh the view; failures keep a short visible line. */
export type ListFeedback = { failed: boolean; text: string }
