import "@tanstack/react-start/server-only"
import { requestOpenHome } from "./openhome.server"
import {
  activeRouteRead,
  unavailableActiveRoute,
  type ActiveRouteRead,
  type RouteSelectionRead,
  loopRead,
  loopRejection,
  unavailableLoop,
  type LoopRead,
  type LoopRequest,
} from "./route-loop"

/** Ask Axum for loops using server-held credentials; only Axum reaches the route planner. */
export async function generateLoops(request: LoopRequest): Promise<LoopRead> {
  const result = await requestOpenHome({
    path: "/api/routes/loops",
    method: "POST",
    // Axum starts no attempt after 20 seconds and allows each 10; leave room for its response.
    timeoutMs: 40_000,
    json: request,
  })

  if (result.state !== "received") return unavailableLoop

  try {
    const body: unknown = await result.response.json()

    // Axum explains targets outside its supported ranges.
    if (result.response.status === 400) {
      const rejection = loopRejection.safeParse(body)

      return rejection.success
        ? { state: "rejected", message: rejection.data.error }
        : unavailableLoop
    }

    if (!result.response.ok) return unavailableLoop

    const parsed = loopRead.safeParse(body)

    return parsed.success ? parsed.data : unavailableLoop
  } catch {
    return unavailableLoop
  }
}

/** Read the selected route from Axum even when its routing provider is offline. */
export async function readActiveRoute(): Promise<ActiveRouteRead> {
  const result = await requestOpenHome({
    path: "/api/routes/active",
    method: "GET",
    timeoutMs: 8000,
  })

  if (result.state !== "received" || !result.response.ok)
    return unavailableActiveRoute

  try {
    const parsed = activeRouteRead.safeParse(await result.response.json())

    return parsed.success ? parsed.data : unavailableActiveRoute
  } catch {
    return unavailableActiveRoute
  }
}

/** Select an Axum-generated identity; no client geometry is sent or trusted. */
export async function selectActiveRoute(selection: {
  route_id: string
}): Promise<RouteSelectionRead> {
  const result = await requestOpenHome({
    path: "/api/routes/active",
    method: "PUT",
    timeoutMs: 8000,
    json: selection,
  })

  if (result.state === "received") {
    if (result.response.status === 409)
      return { state: "failed", reason: "candidate-unavailable" }

    if (result.response.ok) {
      try {
        const parsed = activeRouteRead.safeParse(await result.response.json())

        if (parsed.success && parsed.data.state === "selected")
          return parsed.data
      } catch {
        // A malformed acknowledgement cannot confirm a persisted selection.
      }
    }
  }

  return { state: "failed", reason: "connection-unavailable" }
}
