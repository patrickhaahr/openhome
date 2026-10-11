import "@tanstack/react-start/server-only"
import { z } from "zod"
import type { ConnectionHealth, ConnectionState } from "./connection"

const healthResponse = z.object({ status: z.literal("ok") })

type PrivateConnection = {
  baseUrl: URL
  headers: Headers
}

function parseConfiguration():
  | { state: "ready"; connection: PrivateConnection }
  | { state: "missing-configuration" | "invalid-configuration" } {
  const baseUrl = process.env.OPENHOME_API_URL?.trim()
  const apiKey = process.env.API_KEY?.trim()

  if (
    baseUrl === undefined ||
    baseUrl === "" ||
    apiKey === undefined ||
    apiKey === ""
  ) {
    return { state: "missing-configuration" }
  }

  try {
    const url = new URL(baseUrl)

    if (
      (url.protocol !== "http:" && url.protocol !== "https:") ||
      url.username !== "" ||
      url.password !== "" ||
      url.search !== "" ||
      url.hash !== "" ||
      /\s/.test(apiKey)
    ) {
      return { state: "invalid-configuration" }
    }

    return {
      state: "ready",
      connection: {
        baseUrl: url,
        // Headers encapsulates the credential; only fetch receives it.
        headers: new Headers({ Authorization: `Bearer ${apiKey}` }),
      },
    }
  } catch {
    return { state: "invalid-configuration" }
  }
}

type OpenHomeResponse =
  | { state: "received"; response: Response }
  | { state: Exclude<ConnectionState, "connected" | "unavailable"> }

/** Shared private transport; callers parse responses and serialize only safe outcomes. */
export async function requestOpenHome({
  path,
  method,
  timeoutMs,
}: {
  path: string
  method: "GET" | "POST"
  timeoutMs: number
}): Promise<OpenHomeResponse> {
  const config = parseConfiguration()

  if (config.state !== "ready") return config

  const { baseUrl, headers } = config.connection
  // Preserve an installer-supplied deployment prefix, with or without a trailing slash.
  baseUrl.pathname = `${baseUrl.pathname.replace(/\/$/, "")}${path}`

  try {
    const response = await fetch(baseUrl, {
      method,
      headers,
      cache: "no-store",
      redirect: "error",
      signal: AbortSignal.timeout(timeoutMs),
    })

    if (response.status === 401 || response.status === 403) {
      return { state: "rejected-credential" }
    }

    return { state: "received", response }
  } catch {
    // Transport errors can contain private URLs. Never serialize or log the raw error.
    return { state: "unreachable" }
  }
}

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
