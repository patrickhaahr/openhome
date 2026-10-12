import { createServerFn } from "@tanstack/react-start"
import { setResponseHeader } from "@tanstack/react-start/server"
import {
  loopRequest,
  unavailableLoop,
  routeSelection,
  unavailableActiveRoute,
  type ActiveRouteRead,
  type RouteSelectionRead,
  type LoopRead,
} from "./route-loop"

/** Plan loops through Axum; the API key and route planner stay server-side. */
export const planLoops = createServerFn({ method: "POST" })
  .validator(loopRequest)
  .handler(async ({ data }) => {
    setResponseHeader("Cache-Control", "no-store")
    const { generateLoops } = await import("./route-loop.server")

    return generateLoops(data)
  })

/** A dropped RPC renders recovery guidance instead of an error boundary. */
export async function requestLoops(
  request: Parameters<typeof planLoops>[0]["data"]
): Promise<LoopRead> {
  try {
    return await planLoops({ data: request })
  } catch {
    return unavailableLoop
  }
}

/** Restore the selected route through Start's credential-holding server boundary. */
export const getActiveRoute = createServerFn({ method: "GET" }).handler(
  async () => {
    setResponseHeader("Cache-Control", "no-store")
    const { readActiveRoute } = await import("./route-loop.server")

    return readActiveRoute()
  }
)

/** Persist a deliberate choice through Axum; comparison never writes. */
export const chooseRoute = createServerFn({ method: "POST" })
  .validator(routeSelection)
  .handler(async ({ data }) => {
    setResponseHeader("Cache-Control", "no-store")
    const { selectActiveRoute } = await import("./route-loop.server")

    return selectActiveRoute(data)
  })

/** Failed RPCs keep route planning and recovery reachable. */
export async function loadActiveRoute(): Promise<ActiveRouteRead> {
  try {
    return await getActiveRoute()
  } catch {
    return unavailableActiveRoute
  }
}

/** A dropped selection RPC never claims that a route was saved. */
export async function requestRouteSelection(
  routeId: string
): Promise<RouteSelectionRead> {
  try {
    return await chooseRoute({ data: { route_id: routeId } })
  } catch {
    return { state: "failed", reason: "connection-unavailable" }
  }
}
