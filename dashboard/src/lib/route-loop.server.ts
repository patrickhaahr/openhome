import "@tanstack/react-start/server-only"
import { requestOpenHome } from "./openhome.server"
import {
  loopRead,
  unavailableLoop,
  type LoopRead,
  type WalkingLoopRequest,
} from "./route-loop"

/** Ask Axum for a walking loop using server-held credentials; the provider key stays in Axum. */
export async function generateLoop(
  request: WalkingLoopRequest
): Promise<LoopRead> {
  const result = await requestOpenHome({
    path: "/api/routes/loops",
    method: "POST",
    // Axum allows the provider 15 seconds; leave room for its response.
    timeoutMs: 20_000,
    json: request,
  })

  if (result.state !== "received" || !result.response.ok) return unavailableLoop

  try {
    const parsed = loopRead.safeParse(await result.response.json())

    return parsed.success ? parsed.data : unavailableLoop
  } catch {
    return unavailableLoop
  }
}
