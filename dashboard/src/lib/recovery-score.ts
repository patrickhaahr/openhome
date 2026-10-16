/**
 * Display bands for Axum's recovery measurements. Axum owns every value; these only choose a
 * colour and a ring fill so a glance reads "good, fair or poor" without words.
 */

/** How a value reads at a glance; `none` is a value Axum did not record. */
export type Band = "good" | "fair" | "poor" | "none"

/** Sleep shown as a full Rest ring: eight hours, a display reference rather than a sleep need. */
export const restReferenceMin = 480

/**
 * Band a NOOP Recovery score with WHOOP's thresholds: green from 67, yellow from 34.
 *
 * @param score - Recovery on a 0 to 100 scale, or null when it was not recorded.
 * @returns The score's band.
 */
export function chargeBand(score: number | null): Band {
  if (score === null) return "none"

  if (score >= 67) return "good"

  return score >= 34 ? "fair" : "poor"
}

/**
 * Band a night's sleep against the eight-hour reference: good from 85 %, fair from 70 %.
 *
 * @param minutes - Total sleep in minutes, or null when it was not recorded.
 * @returns The night's band.
 */
export function restBand(minutes: number | null): Band {
  if (minutes === null) return "none"

  const share = minutes / restReferenceMin

  if (share >= 0.85) return "good"

  return share >= 0.7 ? "fair" : "poor"
}

/**
 * The share of a ring to fill, clamped so out-of-range values never overdraw it.
 *
 * @param value - The value, or null when it was not recorded.
 * @param full - The value that fills the ring.
 * @returns A fraction from 0 through 1.
 */
export function ringShare(value: number | null, full: number): number {
  if (value === null || full <= 0) return 0

  return Math.min(1, Math.max(0, value / full))
}

/**
 * Minutes as hours and minutes, the way a sleep duration is read ("7:05").
 *
 * @param minutes - A non-negative duration in minutes.
 * @returns The duration as `h:mm`.
 */
export function clockDuration(minutes: number): string {
  const rounded = Math.round(minutes)

  return `${Math.floor(rounded / 60)}:${String(rounded % 60).padStart(2, "0")}`
}

/**
 * Minutes as a compact duration ("1h 05m", "32m") for stage and session values.
 *
 * @param minutes - A non-negative duration in minutes.
 * @returns The compact duration.
 */
export function shortDuration(minutes: number): string {
  const rounded = Math.round(minutes)

  if (rounded < 60) return `${rounded}m`

  return `${Math.floor(rounded / 60)}h ${String(rounded % 60).padStart(2, "0")}m`
}

/**
 * Minutes as a spoken duration for assistive technology ("7 hours 5 minutes").
 *
 * @param minutes - A non-negative duration in minutes.
 * @returns The spoken duration.
 */
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
