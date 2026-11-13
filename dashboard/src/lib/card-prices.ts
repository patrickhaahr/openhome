import { z } from "zod"
import { cardId } from "./binder"
import { amountKey, formatMoney, type Currency } from "./currency"
import { formatDay } from "./lists"

const amount = z.number().nonnegative()

/** One observed Card Price in USD, converted at its own date's ECB rates; null when none exist yet. */
export const pricePoint = z.object({
  observed_on: z.iso.date(),
  usd: amount,
  eur: amount.nullable(),
  dkk: amount.nullable(),
  rate_date: z.iso.date().nullable(),
})

export type PricePoint = z.infer<typeof pricePoint>

/** A Card's Price History, oldest first. */
export const cardPrices = z.object({
  card_id: cardId,
  prices: pricePoint.array(),
})

export type CardPrices = z.infer<typeof cardPrices>

/** A history is read once per open card sheet; a failed read never means "no history". */
export type PricesState =
  | { readonly state: "loading" }
  | { readonly state: "read"; readonly points: readonly PricePoint[] }
  | { readonly state: "unavailable" }

const dayMs = 86_400_000

function time(date: string): number {
  return Date.parse(`${date}T00:00:00Z`)
}

function dateOf(ms: number): string {
  return new Date(ms).toISOString().slice(0, 10)
}

/** What the chart draws: one slot per calendar day from the first to the last observation. */
export type PriceLine = {
  readonly labels: string[]
  readonly names: string[]
  readonly values: Array<number | null>
  /** What a null slot means: no Price Point that day, or one with no rate in the currency. */
  readonly gaps: string[]
  readonly observations: number
  readonly missing: number
  readonly latest: { readonly day: string; readonly value: number } | undefined
}

/**
 * The points as a daily series in `currency`. A day with no observation, or with no rate yet,
 * is a null slot, so the line breaks there: nothing is interpolated. `gaps` says which.
 */
export function priceLine(
  points: readonly PricePoint[],
  currency: Currency
): PriceLine {
  const first = points[0]
  const last = points.at(-1)

  if (first === undefined || last === undefined)
    return {
      labels: [],
      names: [],
      values: [],
      gaps: [],
      observations: 0,
      missing: 0,
      latest: undefined,
    }

  const byDay = new Map(points.map((point) => [point.observed_on, point]))
  const labels: string[] = []
  const names: string[] = []
  const values: Array<number | null> = []
  const gaps: string[] = []

  for (
    let ms = time(first.observed_on);
    ms <= time(last.observed_on);
    ms += dayMs
  ) {
    const day = dateOf(ms)
    const point = byDay.get(day)
    labels.push(formatDay(day).replace(/ \d{4}$/, ""))
    names.push(formatDay(day))
    values.push(
      point === undefined ? null : (point[amountKey[currency]] ?? null)
    )
    gaps.push(point === undefined ? "No observation" : "No exchange rate")
  }

  const latest = values.reduce<number>(
    (last, value, index) => (value === null ? last : index),
    -1
  )

  return {
    labels,
    names,
    values,
    gaps,
    observations: points.length,
    missing: values.length - points.length,
    latest:
      latest < 0
        ? undefined
        : { day: names[latest] ?? "", value: values[latest] ?? 0 },
  }
}

/** The one line read before the chart. */
export function priceSummary(line: PriceLine, currency: Currency): string {
  const { latest } = line

  if (latest === undefined) return "No price in this currency yet."

  const gaps =
    line.missing === 0
      ? ""
      : `, ${line.missing} ${line.missing === 1 ? "day" : "days"} without an observation`

  return `${formatMoney(latest.value, currency)} on ${latest.day}; ${line.observations} ${line.observations === 1 ? "observation" : "observations"} since ${line.names[0] ?? ""}${gaps}.`
}
