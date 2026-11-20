export type Band = "good" | "fair" | "poor" | "none"

export const restReferenceMin = 480

export function chargeBand(score: number | null): Band {
  if (score === null) return "none"

  if (score >= 67) return "good"

  return score >= 34 ? "fair" : "poor"
}

export function restBand(minutes: number | null): Band {
  if (minutes === null) return "none"

  const share = minutes / restReferenceMin

  if (share >= 0.85) return "good"

  return share >= 0.7 ? "fair" : "poor"
}

export function ringShare(value: number | null, full: number): number {
  if (value === null || full <= 0) return 0

  return Math.min(1, Math.max(0, value / full))
}

export function clockDuration(minutes: number): string {
  const rounded = Math.round(minutes)

  return `${Math.floor(rounded / 60)}:${String(rounded % 60).padStart(2, "0")}`
}

export function shortDuration(minutes: number): string {
  const rounded = Math.round(minutes)

  if (rounded < 60) return `${rounded}m`

  return `${Math.floor(rounded / 60)}h ${String(rounded % 60).padStart(2, "0")}m`
}

export function spokenDuration(minutes: number): string {
  const rounded = Math.round(minutes)
  const hours = Math.floor(rounded / 60)
  const rest = rounded % 60

  const parts = [
    hours === 0 ? "" : `${hours} ${hours === 1 ? "hour" : "hours"}`,
    rest === 0 && hours > 0
      ? ""
      : `${rest} ${rest === 1 ? "minute" : "minutes"}`,
  ]

  return parts.filter((part) => part !== "").join(" ")
}
