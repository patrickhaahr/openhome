import { z } from "zod"
import { shiftDay } from "../training"

export const json = z.json()

export type Json = z.infer<typeof json>

const text = z.string()

const zone = "Europe/Copenhagen"

const unshiftedKeys: ReadonlySet<string> = new Set(["birth_date"])

const dayText = /^\d{4}-\d{2}-\d{2}$/

const instantText =
  /^(\d{4}-\d{2}-\d{2})T(\d{2}:\d{2}:\d{2}(?:\.\d+)?)(Z|[+-]\d{2}:\d{2})$/

const wallClock = new Intl.DateTimeFormat("en-CA", {
  timeZone: zone,
  hourCycle: "h23",
  year: "numeric",
  month: "2-digit",
  day: "2-digit",
  hour: "2-digit",
  minute: "2-digit",
  second: "2-digit",
})

const offsetClock = new Intl.DateTimeFormat("en-GB", {
  timeZone: zone,
  timeZoneName: "longOffset",
})

const msPerMinute = 60_000

function offsetMinutesAt(utcMs: number): number {
  const label =
    offsetClock
      .formatToParts(utcMs)
      .find((part) => part.type === "timeZoneName")?.value ?? "GMT"

  const match = /^GMT(?:([+-])(\d{2}):(\d{2}))?$/.exec(label)

  if (match === null) throw new Error(`Unreadable zone offset ${label}`)

  const [, sign, hours, minutes] = match

  if (sign === undefined) return 0

  return (sign === "-" ? -1 : 1) * (Number(hours) * 60 + Number(minutes))
}

function offsetText(minutes: number): string {
  const sign = minutes < 0 ? "-" : "+"
  const hours = String(Math.floor(Math.abs(minutes) / 60)).padStart(2, "0")
  const rest = String(Math.abs(minutes) % 60).padStart(2, "0")

  return `${sign}${hours}:${rest}`
}

function offsetOfLocal(day: string, time: string): number {
  const asUtc = Date.parse(`${day}T${time}Z`)
  const first = offsetMinutesAt(asUtc - offsetMinutesAt(asUtc) * msPerMinute)

  return offsetMinutesAt(asUtc - first * msPerMinute)
}

function shiftInstant(value: string, days: number): string {
  const matched = instantText.exec(value)

  if (matched === null) return value
  const instant = Date.parse(value)

  if (Number.isNaN(instant)) return value
  const fraction = /\.\d+/.exec(value)?.[0] ?? ""

  const parts = Object.fromEntries(
    wallClock
      .formatToParts(instant)
      .map((part) => [part.type, part.value] as const)
  )

  const day = `${parts["year"]}-${parts["month"]}-${parts["day"]}`
  const time = `${parts["hour"]}:${parts["minute"]}:${parts["second"]}${fraction}`
  const moved = shiftDay(day, days)

  return `${moved}T${time}${offsetText(offsetOfLocal(moved, time))}`
}

export function shiftText(value: string, days: number): string {
  if (days === 0) return value

  if (dayText.test(value)) return shiftDay(value, days)

  return shiftInstant(value, days)
}

export function mapJson(
  body: Json,
  visit: (key: string, value: Json) => Json
): Json {
  return json.parse(
    JSON.parse(JSON.stringify(body), (key: string, value: Json) =>
      visit(key, value)
    )
  )
}

export function shiftJson(body: Json, days: number): Json {
  return mapJson(body, (key, value) => {
    const found = text.safeParse(value)

    return found.success && !unshiftedKeys.has(key)
      ? shiftText(found.data, days)
      : value
  })
}

export function wholeDaysBetween(from: string, to: string): number {
  return Math.round(
    (Date.parse(`${to}T00:00:00Z`) - Date.parse(`${from}T00:00:00Z`)) /
      86_400_000
  )
}

export function isNormalForm(value: string): boolean {
  const matched = instantText.exec(value)

  if (matched === null) return true
  const instant = Date.parse(value)

  return (
    !Number.isNaN(instant) &&
    offsetText(offsetMinutesAt(instant)) === matched[3]
  )
}
