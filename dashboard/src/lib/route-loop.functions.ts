import { createServerFn } from "@tanstack/react-start"
import { setResponseHeader } from "@tanstack/react-start/server"
import { loopRequest, unavailableLoop, type LoopRead } from "./route-loop"

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
