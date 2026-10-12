import "@tanstack/react-start/server-only"
import { requestOpenHome } from "./openhome.server"
import { mapOriginRead, unavailableMapOrigin, type MapOriginRead } from "./map"

/** Read the private map origin from Axum using server-held credentials. */
export async function readMapOrigin(): Promise<MapOriginRead> {
  const result = await requestOpenHome({
    path: "/api/map",
    method: "GET",
    timeoutMs: 4000,
  })

  if (result.state !== "received" || !result.response.ok)
    return unavailableMapOrigin

  try {
    const parsed = mapOriginRead.safeParse(await result.response.json())

    return parsed.success ? parsed.data : unavailableMapOrigin
  } catch {
    return unavailableMapOrigin
  }
}
