import "@tanstack/react-start/server-only"
import { z } from "zod"
import type { ConnectionState } from "./connection"

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
  json,
}: {
  path: `/api/${string}`
  method: "GET" | "POST" | "PATCH" | "DELETE"
  timeoutMs: number
  json?: Record<string, string>
}): Promise<OpenHomeResponse> {
  const config = parseConfiguration()

  if (config.state !== "ready") return config

  const { baseUrl, headers } = config.connection
  // Preserve an installer-supplied deployment prefix, with or without a trailing slash.
  baseUrl.pathname = `${baseUrl.pathname.replace(/\/$/, "")}${path}`

  if (json !== undefined) headers.set("Content-Type", "application/json")

  try {
    const response = await fetch(baseUrl, {
      method,
      headers,
      body: json === undefined ? null : JSON.stringify(json),
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

const acknowledgement = z.object({ message: z.string() })

const errorBody = z.object({ error: z.string() })

/** Whether Axum accepted a command; any other body cannot confirm it. */
export async function isAcknowledged(response: Response): Promise<boolean> {
  if (!response.ok) return false

  try {
    return acknowledgement.safeParse(await response.json()).success
  } catch {
    return false
  }
}

/**
 * Read Axum's error message for exact classification only.
 *
 * @returns The message, or undefined when the body is not Axum's error shape. Never serialize it.
 */
export async function readErrorMessage(
  response: Response
): Promise<string | undefined> {
  try {
    const parsed = errorBody.safeParse(await response.json())

    return parsed.success ? parsed.data.error : undefined
  } catch {
    return undefined
  }
}
