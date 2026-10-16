/**
 * Where the sun is over central Copenhagen. A public city-level reference point, never the
 * household's private location: the dashboard already shows Copenhagen time.
 */
const latitude = 55.676

const longitude = 12.568

const radians = Math.PI / 180

/** Sunrise and sunset for one civil day, with the sun's current progress across it. */
export type Daylight = {
  /** Copenhagen sunrise, or null in polar day/night (never at this latitude, but typed honestly). */
  readonly sunrise: Date | null
  readonly sunset: Date | null
  /** Solar altitude in degrees; negative below the horizon. */
  readonly altitude: number
  /** 0 at sunrise, 1 at sunset; below 0 or above 1 at night. */
  readonly progress: number
  /** The theme the room's light calls for: dusk begins when the sun is 4° above the horizon. */
  readonly phase: "day" | "night"
}

function julianDay(at: Date): number {
  return at.getTime() / 86_400_000 + 2440587.5
}

/** Solar declination (°) and equation of time (minutes), NOAA low-precision formulas. */
function solarPosition(at: Date) {
  const century = (julianDay(at) - 2451545) / 36525

  const meanLongitude =
    (280.46646 + century * (36000.76983 + century * 0.0003032)) % 360

  const meanAnomaly = 357.52911 + century * (35999.05029 - 0.0001537 * century)

  const eccentricity =
    0.016708634 - century * (0.000042037 + 0.0000001267 * century)

  const center =
    Math.sin(meanAnomaly * radians) *
      (1.914602 - century * (0.004817 + 0.000014 * century)) +
    Math.sin(2 * meanAnomaly * radians) * (0.019993 - 0.000101 * century) +
    Math.sin(3 * meanAnomaly * radians) * 0.000289

  const omega = 125.04 - 1934.136 * century

  const apparent =
    meanLongitude + center - 0.00569 - 0.00478 * Math.sin(omega * radians)

  const obliquity =
    23 +
    (26 +
      (21.448 - century * (46.815 + century * (0.00059 - century * 0.001813))) /
        60) /
      60 +
    0.00256 * Math.cos(omega * radians)

  const declination =
    Math.asin(Math.sin(obliquity * radians) * Math.sin(apparent * radians)) /
    radians

  const y = Math.tan((obliquity / 2) * radians) ** 2

  const equation =
    (4 *
      (y * Math.sin(2 * meanLongitude * radians) -
        2 * eccentricity * Math.sin(meanAnomaly * radians) +
        4 *
          eccentricity *
          y *
          Math.sin(meanAnomaly * radians) *
          Math.cos(2 * meanLongitude * radians) -
        0.5 * y * y * Math.sin(4 * meanLongitude * radians) -
        1.25 *
          eccentricity *
          eccentricity *
          Math.sin(2 * meanAnomaly * radians))) /
    radians

  return { declination, equation }
}

function altitudeAt(at: Date): number {
  const { declination, equation } = solarPosition(at)

  const minutes =
    at.getUTCHours() * 60 + at.getUTCMinutes() + at.getUTCSeconds() / 60

  const solarTime = (minutes + equation + 4 * longitude + 1440) % 1440
  const hourAngle = solarTime / 4 - 180

  const cosZenith =
    Math.sin(latitude * radians) * Math.sin(declination * radians) +
    Math.cos(latitude * radians) *
      Math.cos(declination * radians) *
      Math.cos(hourAngle * radians)

  return 90 - Math.acos(Math.min(1, Math.max(-1, cosZenith))) / radians
}

const copenhagenDate = new Intl.DateTimeFormat("en-CA", {
  timeZone: "Europe/Copenhagen",
})

/** Sunrise or sunset on the Copenhagen calendar day containing `at`, refraction-corrected. */
function crossing(at: Date, kind: "rise" | "set"): Date | null {
  const noon = new Date(`${copenhagenDate.format(at)}T12:00:00Z`)
  const { declination, equation } = solarPosition(noon)

  const cosHour =
    Math.cos(90.833 * radians) /
      (Math.cos(latitude * radians) * Math.cos(declination * radians)) -
    Math.tan(latitude * radians) * Math.tan(declination * radians)

  if (cosHour < -1 || cosHour > 1) return null

  const hour = Math.acos(cosHour) / radians

  const minutes =
    720 - 4 * (longitude + (kind === "rise" ? hour : -hour)) - equation

  return new Date(noon.getTime() + (minutes - 720) * 60_000)
}

/** The sun over Copenhagen at `at`. */
export function daylight(at: Date): Daylight {
  const sunrise = crossing(at, "rise")
  const sunset = crossing(at, "set")
  const altitude = altitudeAt(at)

  const progress =
    sunrise === null || sunset === null
      ? altitude > 0
        ? 0.5
        : -0.5
      : (at.getTime() - sunrise.getTime()) /
        (sunset.getTime() - sunrise.getTime())

  return {
    sunrise,
    sunset,
    altitude,
    progress,
    phase: altitude > 4 ? "day" : "night",
  }
}
