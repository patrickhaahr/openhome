import { z } from "zod"

const id = z.uuid().optional().catch(undefined)

export const personSearch = z.object({ person: id })

export const wishSearch = personSearch.extend({ list: id })

export const listsSearch = wishSearch.extend({
  tab: z.enum(["wish", "watch", "dates"]).optional().catch(undefined),
})

export type PersonSearch = z.infer<typeof personSearch>

export type WishSearch = z.infer<typeof wishSearch>

const dayFormat = new Intl.DateTimeFormat("en-GB", {
  day: "numeric",
  month: "short",
  year: "numeric",
  timeZone: "UTC",
})

export function formatDay(date: string): string {
  return dayFormat.format(new Date(`${date}T00:00:00Z`))
}

export function initial(name: string): string {
  return Array.from(name.trim())[0]?.toLocaleUpperCase("en-GB") ?? "?"
}

export function trimText(value: string): string {
  return value.replace(/^\p{White_Space}+|\p{White_Space}+$/gu, "")
}

export type ListFeedback = { failed: boolean; text: string }
