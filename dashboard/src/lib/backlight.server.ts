import "@tanstack/react-start/server-only"
import { requestOpenHome } from "./openhome.server"
import {
  backlightStatus,
  type BacklightAction,
  type BacklightRead,
  type BacklightResult,
} from "./backlight"

async function parseStatus(response: Response): Promise<BacklightRead> {
  try {
    const parsed = backlightStatus.safeParse(await response.json())

    if (parsed.success) return { state: "available", status: parsed.data }
  } catch {
    /* Never serialize private transport or parsing details. */
  }

  return { state: "unavailable" }
}

export async function readBacklight(): Promise<BacklightRead> {
  const result = await requestOpenHome({
    path: "/api/backlight",
    method: "GET",
    timeoutMs: 7000,
  })

  if (result.state !== "received" || !result.response.ok)
    return { state: "unavailable" }

  return parseStatus(result.response)
}

export async function changeBacklight(
  action: BacklightAction
): Promise<BacklightResult> {
  let path: `/api/${string}` = "/api/backlight/refresh"
  let method: "POST" | "PUT" = "POST"

  let json:
    | { mode: "off" | "tv-sync" }
    | { source: "capture" | "backup" }
    | undefined

  switch (action.action) {
    case "select":
      path = "/api/backlight/selection"
      method = "PUT"
      json = action.selection
      break
    case "profile":
      path = "/api/backlight/profile"
      json = { source: action.source }
      break
    case "refresh":
      break
  }

  const request: Parameters<typeof requestOpenHome>[0] = {
    path,
    method,
    timeoutMs: 65000,
  }

  if (json !== undefined) request.json = json

  const result = await requestOpenHome(request)

  if (result.state !== "received") return { state: "unavailable" }

  if (result.response.status === 409) return { state: "profile-required" }

  if (result.response.status === 400) return { state: "invalid-profile" }

  if (!result.response.ok) return { state: "unavailable" }

  return parseStatus(result.response)
}
