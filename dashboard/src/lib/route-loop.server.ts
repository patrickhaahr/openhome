import "@tanstack/react-start/server-only"
import { requestOpenHome } from "./openhome.server"
import {
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
