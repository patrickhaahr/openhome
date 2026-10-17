import { z } from "zod"
import type { HouseholdUser } from "./wishlist"

const count = z.number().int().nonnegative()

/** Axum's milestone decision; display code only formats it. */
const milestone = z.discriminatedUnion("kind", [
  z.object({ kind: z.literal("anniversary"), years: count }),
  z.object({ kind: z.literal("day_count"), days: count }),
  z.object({ kind: z.literal("yearly"), years: count }),
])

/** One household-shared date with the figures Axum computed for the Copenhagen day. */
export const importantDate = z.object({
  id: z.uuid(),
  title: z.string(),
  date: z.iso.date(),
  kind: z.enum(["since", "yearly"]),
  person_ids: z.array(z.uuid()),
  show_when_idle: z.boolean(),
  elapsed: z
    .object({
      years: count,
      months: count,
      days: count,
      total_days: count,
    })
    .nullable(),
  next: z.object({ date: z.iso.date(), days_until: count, years: count }),
  milestone: milestone.nullable().default(null),
})

/** A stored date and its figures; the dashboard never does day arithmetic itself. */
export type ImportantDate = z.infer<typeof importantDate>

/** Every date for one day, in Axum's order: since dates first, then yearly by next occurrence. */
export const importantDates = z.object({
  on: z.iso.date(),
  dates: z.array(importantDate),
})

/** Sanitized transport and API outcomes; private diagnostics never reach the display. */
export type ImportantDateFailure =
  | "missing-configuration"
  | "invalid-configuration"
  | "rejected-credential"
  | "unreachable"
  | "unavailable"
  | "invalid"
  | "conflict"
  | "not-found"

/** An incomplete read never looks like an empty list. */
export type ImportantDatesState =
  | {
      state: "available"
      owners: HouseholdUser[]
      on: string
      dates: ImportantDate[]
    }
  | { state: "unavailable"; reason: ImportantDateFailure }

const fields = {
  title: z.string(),
  date: z.iso.date(),
  kind: importantDate.shape.kind,
  person_ids: z.array(z.uuid()),
  show_when_idle: z.boolean(),
}

/** Writes name every tag explicitly; Axum owns validation and retry identity. */
export const importantDateCommand = z.discriminatedUnion("action", [
  z.object({ action: z.literal("add"), ...fields, request_id: z.uuid() }),
  z.object({
    action: z.literal("edit"),
    date_id: z.uuid(),
    title: fields.title.optional(),
    date: fields.date.optional(),
    kind: fields.kind.optional(),
    person_ids: fields.person_ids.optional(),
    show_when_idle: fields.show_when_idle.optional(),
  }),
  z.object({ action: z.literal("remove"), date_id: z.uuid() }),
])

export type ImportantDateCommand = z.infer<typeof importantDateCommand>

/** Confirmation of persistence, separate from uncertain network outcomes. */
export type ImportantDateMutation =
  | { state: "saved" }
  | { state: "failed"; reason: ImportantDateFailure }

/** Actionable copy without household configuration or upstream error bodies. */
export const importantDateFailureMessage: Record<ImportantDateFailure, string> =
  {
    "missing-configuration":
      "Connect OpenHome in Connection before opening dates.",
    "invalid-configuration": "Check the private OpenHome setup in Connection.",
    "rejected-credential": "OpenHome rejected the API key. Check Connection.",
    unreachable:
      "The response was lost. Reopen the page to check whether the change saved. Adding the same date again will not duplicate it.",
    unavailable:
      "OpenHome could not confirm the result. Reopen the page before trying again.",
    invalid: "Check the title and the date. The date must be a real day.",
    conflict:
      "This date was changed or removed elsewhere. Reopen the page to see the list before retrying.",
    "not-found":
      "This date or person is no longer here. Reopen the page to see the current dates.",
  }

function plural(value: number, one: string, many: string): string {
  return `${value} ${value === 1 ? one : many}`
}

/** The day a figure counts down to: "Today", "Tomorrow" or "in 18 days". */
export function countdown(days: number): string {
  if (days === 0) return "Today"

  if (days === 1) return "Tomorrow"

  return `in ${plural(days, "day", "days")}`
}

/** A date's lead figure in parts: elapsed time for since, the next occurrence for yearly. */
export function figureParts(date: ImportantDate): string[] {
  if (date.kind === "yearly" || date.elapsed === null)
    return [countdown(date.next.days_until)]

  const { years, months, days } = date.elapsed

  const parts = [
    years > 0 ? plural(years, "year", "years") : undefined,
    months > 0 ? plural(months, "month", "months") : undefined,
    days > 0 || (years === 0 && months === 0)
      ? plural(days, "day", "days")
      : undefined,
  ].filter((part) => part !== undefined)

  return parts
}

/** The lead figure as one line, e.g. "2 years · 3 months · 12 days" or "in 18 days". */
export function leadFigure(date: ImportantDate): string {
  return figureParts(date).join(" · ")
}

/** A milestone's lead line; yearly titles stay literal because dates carry no birthday type. */
export function milestoneFigure(date: ImportantDate): string | undefined {
  const milestone = date.milestone

  if (milestone === null) return undefined

  switch (milestone.kind) {
    case "anniversary":
      return `${plural(milestone.years, "year", "years")} today`
    case "day_count":
      return `${milestone.days.toLocaleString("en-GB")} days today`
    case "yearly":
      return `${date.title} today`
  }
}

const milestonePriority = { anniversary: 0, day_count: 1, yearly: 2 }

/** Select one idle-safe lead: anniversary, round day count, then yearly; ties keep API order. */
export function idleMilestone(
  dates: readonly ImportantDate[]
): ImportantDate | undefined {
  let lead: ImportantDate | undefined

  for (const date of dates) {
    if (!date.show_when_idle || date.milestone === null) continue

    if (
      lead === undefined ||
      lead.milestone === null ||
      milestonePriority[date.milestone.kind] <
        milestonePriority[lead.milestone.kind]
    )
      lead = date
  }

  return lead
}

const today = new Intl.DateTimeFormat("en-CA", {
  timeZone: "Europe/Copenhagen",
  year: "numeric",
  month: "2-digit",
  day: "2-digit",
})

/** The Europe/Copenhagen calendar day of an instant, as YYYY-MM-DD; used only to re-read. */
export function copenhagenDay(instant: Date): string {
  return today.format(instant)
}

/** The shared person filter: that person's tagged dates, plus the dates that tag no one. */
export function datesFor(
  dates: readonly ImportantDate[],
  person: string | undefined
): ImportantDate[] {
  return dates.filter(
    (date) =>
      person === undefined ||
      date.person_ids.length === 0 ||
      date.person_ids.includes(person)
  )
}
