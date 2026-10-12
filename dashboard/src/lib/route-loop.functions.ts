import { createServerFn } from "@tanstack/react-start"
import { setResponseHeader } from "@tanstack/react-start/server"
import {
  unavailableLoop,
  walkingLoopRequest,
  type LoopRead,
} from "./route-loop"

/** Plan a walking loop through Axum; the API key, provider key and origin stay server-side. */
export const planWalkingLoop = createServerFn({ method: "POST" })
  .validator(walkingLoopRequest)
  .handler(async ({ data }) => {
    setResponseHeader("Cache-Control", "no-store")
    const { generateLoop } = await import("./route-loop.server")

    return generateLoop(data)
  })

/** A dropped RPC renders recovery guidance instead of an error boundary. */
export async function requestWalkingLoop(
  request: Parameters<typeof planWalkingLoop>[0]["data"]
): Promise<LoopRead> {
  try {
    return await planWalkingLoop({ data: request })
  } catch {
    return unavailableLoop
  }
}
