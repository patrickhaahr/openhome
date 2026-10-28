/** Pure light geometry for the LED page: colour wheel and bedtime dial maths. */

type Rgb = { red: number; green: number; blue: number }

/** Hue in degrees [0, 360), saturation and value in [0, 1]. */
export type Hsv = { hue: number; saturation: number; value: number }

/** The controller's red, used when no colour has been chosen yet. */
export const defaultRgb: Rgb = { red: 255, green: 0, blue: 0 }

/**
 * @param rgb - Channels 0 to 255.
 * @returns The same colour as hue, saturation and value.
 */
export function rgbToHsv({ red, green, blue }: Rgb): Hsv {
  const r = red / 255
  const g = green / 255
  const b = blue / 255
  const max = Math.max(r, g, b)
  const delta = max - Math.min(r, g, b)
  let hue = 0

  if (delta > 0) {
    if (max === r) hue = 60 * (((g - b) / delta) % 6)
    else if (max === g) hue = 60 * ((b - r) / delta + 2)
    else hue = 60 * ((r - g) / delta + 4)
  }

  return {
    hue: (hue + 360) % 360,
    saturation: max === 0 ? 0 : delta / max,
    value: max,
  }
}

/**
 * @param hsv - Hue in degrees, saturation and value in [0, 1].
 * @returns Integer channels 0 to 255.
 */
export function hsvToRgb({ hue, saturation, value }: Hsv): Rgb {
  const h = (((hue % 360) + 360) % 360) / 60
  const c = value * saturation
  const x = c * (1 - Math.abs((h % 2) - 1))
  const m = value - c

  const [r, g, b] =
    h < 1
      ? [c, x, 0]
      : h < 2
        ? [x, c, 0]
        : h < 3
          ? [0, c, x]
          : h < 4
            ? [0, x, c]
            : h < 5
              ? [x, 0, c]
              : [c, 0, x]

  const channel = (v: number): number =>
    Math.min(255, Math.max(0, Math.round((v + m) * 255)))

  return { red: channel(r), green: channel(g), blue: channel(b) }
}

/** CSS colour for a channel triple. */
export function rgbCss({ red, green, blue }: Rgb): string {
  return `rgb(${red} ${green} ${blue})`
}

const minutesPerDay = 24 * 60

/** "HH:MM" to minutes after midnight. */
export function clockMinutes(time: string): number {
  const [hours = 0, minutes = 0] = time.split(":").map(Number)

  return hours * 60 + minutes
}

/** Minutes after midnight (any integer) to "HH:MM". */
export function clockTime(minutes: number): string {
  const wrapped = ((minutes % minutesPerDay) + minutesPerDay) % minutesPerDay

  return `${String(Math.floor(wrapped / 60)).padStart(2, "0")}:${String(wrapped % 60).padStart(2, "0")}`
}

/**
 * Moves one end of the bedtime window by `delta` minutes on a `step` grid, skipping over the
 * other end so the window never collapses to nothing.
 */
export function stepClock(
  time: string,
  delta: number,
  other: string,
  step = 15
): string {
  const snapped = Math.round(clockMinutes(time) / step) * step
  let next = clockTime(snapped + delta)

  if (next === other)
    next = clockTime(snapped + delta + Math.sign(delta) * step)

  return next
}

/** The dial puts midnight at the top and runs clockwise through the day. */
export function clockAngle(minutes: number): number {
  return (minutes / minutesPerDay) * 360
}

/** The time under a dial angle in degrees (0 at the top, clockwise), snapped to `step`. */
export function angleClock(degrees: number, step = 15): string {
  const minutes = (((degrees % 360) + 360) % 360) * (minutesPerDay / 360)

  return clockTime(Math.round(minutes / step) * step)
}

/** A position in the dial's drawing units. */
export type Point = { x: number; y: number }

/** A point on a circle, with 0 degrees at the top and angles running clockwise. */
export function polar(centre: number, radius: number, degrees: number): Point {
  const radians = ((degrees - 90) * Math.PI) / 180

  // Rounded so the server and the browser draw the same attribute.
  return {
    x: Math.round((centre + radius * Math.cos(radians)) * 100) / 100,
    y: Math.round((centre + radius * Math.sin(radians)) * 100) / 100,
  }
}

/** An SVG arc path clockwise from `from` to `to` minutes after midnight, across midnight. */
export function clockArc(
  centre: number,
  radius: number,
  from: number,
  to: number
): string {
  const span = (((to - from) % minutesPerDay) + minutesPerDay) % minutesPerDay
  const start = polar(centre, radius, clockAngle(from))
  const end = polar(centre, radius, clockAngle(from + span))
  const large = span > minutesPerDay / 2 ? 1 : 0

  return `M ${start.x} ${start.y} A ${radius} ${radius} 0 ${large} 1 ${end.x} ${end.y}`
}

const copenhagenClock = new Intl.DateTimeFormat("en-GB", {
  timeZone: "Europe/Copenhagen",
  hour: "2-digit",
  minute: "2-digit",
  hourCycle: "h23",
})

/** Minutes after midnight in Copenhagen at an instant. */
export function copenhagenMinutes(at: Date): number {
  return clockMinutes(copenhagenClock.format(at))
}

/** "HH:MM" in Copenhagen at an instant. */
export function copenhagenClockTime(at: Date): string {
  return copenhagenClock.format(at)
}
