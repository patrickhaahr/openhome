import { z } from "zod"

const current = z.object({
  observed_at: z.iso.datetime({ offset: true }),
  temperature_c: z.number().nullable(),
  feels_like_c: z.number().nullable(),
  weather_code: z.number().int().min(0).max(255).nullable(),
  wind_ms: z.number().nonnegative().nullable(),
})

const day = z.object({
  date: z.iso.date(),
  weather_code: z.number().int().min(0).max(255).nullable(),
  high_c: z.number().nullable(),
  low_c: z.number().nullable(),
  precipitation_mm: z.number().nonnegative().nullable(),
  wind_ms: z.number().nonnegative().nullable(),
})

const forecast = z.object({
  fetched_at: z.iso.datetime({ offset: true }),
  valid_until: z.iso.datetime({ offset: true }),
  timezone: z.literal("Europe/Copenhagen"),
  current: current.nullable(),
  days: z.array(day).length(7),
})

/** The normalized Axum contract, stripping unknown provider and private fields. */
export const weatherRead = z.discriminatedUnion("state", [
  z.object({ state: z.literal("fresh"), forecast }),
  z.object({ state: z.literal("stale"), forecast }),
  z.object({
    state: z.literal("unavailable"),
    reason: z.enum(["missing-configuration", "provider-unavailable"]),
  }),
])

/** Weather available for rendering, or a sanitized connection/provider failure. */
export type WeatherRead = z.infer<typeof weatherRead>

/** A failed transport or RPC must never become fabricated weather. */
export const unavailableWeather: WeatherRead = {
  state: "unavailable",
  reason: "provider-unavailable",
}

/** WMO descriptions; unknown and missing codes never imply clear skies. */
export function weatherDescription(code: number | null): string {
  switch (code) {
    case 0:
      return "Clear sky"
    case 1:
      return "Mainly clear"
    case 2:
      return "Partly cloudy"
    case 3:
      return "Overcast"
    case 45:
    case 48:
      return "Fog"
    case 51:
    case 53:
    case 55:
      return "Drizzle"
    case 56:
    case 57:
      return "Freezing drizzle"
    case 61:
    case 63:
    case 65:
      return "Rain"
    case 66:
    case 67:
      return "Freezing rain"
    case 71:
    case 73:
    case 75:
    case 77:
      return "Snow"
    case 80:
    case 81:
    case 82:
      return "Rain showers"
    case 85:
    case 86:
      return "Snow showers"
    case 95:
    case 96:
    case 99:
      return "Thunderstorm"
    case null:
    default:
      return "Conditions unavailable"
  }
}
