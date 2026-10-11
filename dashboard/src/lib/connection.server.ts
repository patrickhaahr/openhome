import "@tanstack/react-start/server-only"
import { z } from "zod"
import type { ConnectionHealth, ConnectionState } from "./connection"

const healthResponse = z.object({ status: z.literal("ok") })

type PrivateConnection = {
  healthUrl: URL
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

    // Preserve an installer-supplied deployment prefix, with or without a trailing slash.
    url.pathname = `${url.pathname.replace(/\/$/, "")}/api/health`

    return {
      state: "ready",
      connection: {
        healthUrl: url,
        // Headers encapsulates the credential; only fetch receives it.
        headers: new Headers({ Authorization: `Bearer ${apiKey}` }),
      },
    }
  } catch {
    return { state: "invalid-configuration" }
  }
}

async function checkHealth(
  connection: PrivateConnection
): Promise<ConnectionState> {
  try {
    const response = await fetch(connection.healthUrl, {
      headers: connection.headers,
      cache: "no-store",
      redirect: "error",
      signal: AbortSignal.timeout(4000),
    })

    if (response.status === 401 || response.status === 403) {
      return "rejected-credential"
    }

    if (!response.ok) {
      return "unavailable"
    }

    try {
      const body: unknown = await response.json()

      return healthResponse.safeParse(body).success
        ? "connected"
        : "unavailable"
    } catch {
      return "unavailable"
    }
  } catch {
    // Transport errors can contain private URLs. Never serialize or log the raw error.
    return "unreachable"
  }
}

/** Read private runtime configuration and return only an authenticated health outcome. */
export async function readConnectionHealth(): Promise<ConnectionHealth> {
  const config = parseConfiguration()

  const state =
    config.state === "ready"
      ? await checkHealth(config.connection)
      : config.state

  return { state, checkedAt: new Date().toISOString() }
}
