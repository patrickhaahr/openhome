import "@tanstack/react-start/server-only"
import { requestOpenHome } from "./openhome.server"
import { unavailableWeather, weatherRead, type WeatherRead } from "./weather"

/** Read Axum weather using server-held credentials and serialize only the normalized contract. */
export async function readWeather(): Promise<WeatherRead> {
  const result = await requestOpenHome({
    path: "/api/weather",
    method: "GET",
    timeoutMs: 7000,
  })

  if (result.state !== "received" || !result.response.ok)
    return unavailableWeather

  try {
    const parsed = weatherRead.safeParse(await result.response.json())

    return parsed.success ? parsed.data : unavailableWeather
  } catch {
    return unavailableWeather
  }
}
