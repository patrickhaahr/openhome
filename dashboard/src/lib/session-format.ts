const clock = new Intl.DateTimeFormat("en-GB", {
  timeZone: "Europe/Copenhagen",
  hour: "2-digit",
  minute: "2-digit",
  hourCycle: "h23",
})

const longDay = new Intl.DateTimeFormat("en-GB", {
  timeZone: "UTC",
  weekday: "short",
  day: "numeric",
  month: "short",
})

/**
 * A timestamp as the Copenhagen wall clock, such as "07:50".
 *
 * @param instant - An ISO timestamp.
 * @returns The 24-hour time in Europe/Copenhagen.
 */
export function clockTime(instant: string): string {
  return clock.format(new Date(instant))
}

/**
 * Seconds per kilometre as a pace, such as "5:33".
 *
 * @param secondsPerKm - The pace in seconds for one kilometre.
 * @returns Minutes and zero-padded seconds.
 */
export function pace(secondsPerKm: number): string {
  const rounded = Math.round(secondsPerKm)

  return `${Math.floor(rounded / 60)}:${String(rounded % 60).padStart(2, "0")}`
}

/**
 * A Training Day as a short English date, such as "Thu 15 Oct".
 *
 * @param day - A `YYYY-MM-DD` Training Day.
 * @returns The day; formatted at UTC midnight so the calendar date never shifts.
 */
export function dayLabel(day: string): string {
  const date = new Date(`${day}T00:00:00Z`)

  return Number.isNaN(date.getTime()) ? day : longDay.format(date)
}

/** How a session is drawn: the glyph follows the sport, never the other way round. */
export type SportKind = "strength" | "run" | "walk" | "ride" | "other"

/**
 * Group a NOOP sport name for its glyph.
 *
 * @param sport - The NOOP sport name.
 * @returns The kind of movement it is.
 */
export function sportKind(sport: string): SportKind {
  if (/\b(run|running|jog|jogging)\b/i.test(sport)) return "run"

  if (/\b(walk|walking|hike|hiking|trek)\b/i.test(sport)) return "walk"

  if (/\b(cycle|cycling|bike|biking|ride|riding)\b/i.test(sport)) return "ride"

  return /\b(calisthenics|strength|weight|weights|weightlifting|lifting|gym|functional|crossfit)\b/i.test(
    sport
  )
    ? "strength"
    : "other"
}
