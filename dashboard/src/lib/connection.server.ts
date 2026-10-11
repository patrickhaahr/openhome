import "@tanstack/react-start/server-only"
import { z } from "zod"
import type { ConnectionHealth, ConnectionState } from "./connection"
import { requestOpenHome } from "./openhome.server"

const healthResponse = z.object({ status: z.literal("ok") })

async function checkHealth(): Promise<ConnectionState> {
  const result = await requestOpenHome({
    path: "/api/health",
    method: "GET",
    timeoutMs: 4000,
  })

  if (result.state !== "received") return result.state

  if (!result.response.ok) return "unavailable"

  try {
    const body: unknown = await result.response.json()

    return healthResponse.safeParse(body).success ? "connected" : "unavailable"
  } catch {
    return "unavailable"
  }
}

/** Read private runtime configuration and return only an authenticated health outcome. */
export async function readConnectionHealth(): Promise<ConnectionHealth> {
  const state = await checkHealth()

  return { state, checkedAt: new Date().toISOString() }
}
