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

export function clockTime(instant: string): string {
  return clock.format(new Date(instant))
}

export function pace(secondsPerKm: number): string {
  const rounded = Math.round(secondsPerKm)

  return `${Math.floor(rounded / 60)}:${String(rounded % 60).padStart(2, "0")}`
}

export function dayLabel(day: string): string {
  const date = new Date(`${day}T00:00:00Z`)

  return Number.isNaN(date.getTime()) ? day : longDay.format(date)
}

export type SportKind = "strength" | "run" | "walk" | "ride" | "other"

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
